use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use crate::{
    config::{FanConfig, SpeedCurve},
    error::{Error, FanError, Result},
};

macro_rules! write_trunc {
    ($dst:expr, $($arg:tt)*) => {
        Ok(&mut $dst)
            .and_then(|w| {
                use std::io::Seek;
                w.seek(std::io::SeekFrom::Start(0))?;
                w.set_len(0)?;
                write!(w, $($arg)*)
            })
    }
}

#[derive(Debug)]
pub struct FanController {
    path: PathBuf,
    manual_file: std::fs::File,
    output_file: std::fs::File,
    input_file: std::fs::File,
    config: FanConfig,
    current_speed: Option<u32>,
    ramp_start_time: Option<Instant>,
    ramp_start_speed: Option<u32>,

    min_speed: u32,
    max_speed: u32,
}

impl FanController {
    pub fn new(fan: PathBuf, config: FanConfig, temp_buf: &mut String) -> Result<Self> {
        fn open_component(path: PathBuf, component: FanComponent, read_only: bool) -> Result<File> {
            let mut opt = std::fs::OpenOptions::new();
            if read_only {
                opt.read(true);
            } else {
                opt.write(true);
            }
            opt.open(component.to_path(&path))
                .map_err(|e| Error::Fan(path, FanError::Open(component, e)))
        }

        let min_speed =
            open_component(fan.clone(), FanComponent::Min, true).and_then(|mut fan_min| {
                temp_buf.clear();
                fan_min
                    .read_to_string(temp_buf)
                    .map_err(|e| Error::Fan(fan.clone(), FanError::Read(FanComponent::Min, e)))?;
                temp_buf
                    .trim()
                    .parse()
                    .map_err(|e| Error::Fan(fan.clone(), FanError::Parse(FanComponent::Min, e)))
            })?;
        let max_speed =
            open_component(fan.clone(), FanComponent::Max, true).and_then(|mut fan_max| {
                temp_buf.clear();
                fan_max
                    .read_to_string(temp_buf)
                    .map_err(|e| Error::Fan(fan.clone(), FanError::Read(FanComponent::Max, e)))?;
                temp_buf
                    .trim()
                    .parse()
                    .map_err(|e| Error::Fan(fan.clone(), FanError::Parse(FanComponent::Max, e)))
            })?;

        let manual_file = open_component(fan.clone(), FanComponent::Manual, false)?;
        let output_file = open_component(fan.clone(), FanComponent::Output, false)?;
        let input_file = open_component(fan.clone(), FanComponent::Input, true)?;

        let mut this = Self {
            path: fan.clone(),
            manual_file,
            output_file,
            input_file,
            config,
            current_speed: None,
            ramp_start_time: None,
            ramp_start_speed: None,
            min_speed,
            max_speed,
        };
        log::info!(
            "Initialized fan: path={}, min={} RPM, max={} RPM, config={:#?}",
            this.path.display(),
            this.min_speed,
            this.max_speed,
            this.config,
        );

        // Acquire manual control (see `Drop` impl)
        this.set_manual(true).map_err(|e| Error::Fan(fan, e))?;

        Ok(this)
    }

    fn set_manual(&mut self, enabled: bool) -> Result<(), FanError> {
        write_trunc!(&mut self.manual_file, "{}", usize::from(enabled))
            .map_err(|e| FanError::Write(FanComponent::Manual, e))?;
        Ok(())
    }

    pub fn set_speed(&mut self, mut speed: u32) -> Result<bool> {
        speed = speed.clamp(self.min_speed, self.max_speed);

        if self.current_speed == Some(speed) {
            return Ok(false);
        }

        write_trunc!(&mut self.output_file, "{speed}")
            .map_err(|e| Error::Fan(self.path.clone(), FanError::Write(FanComponent::Output, e)))?;

        // Only start a new settling window when direction changes or
        // we're starting from a settled state, not on every incremental step.
        let starting_new_ramp = match (self.current_speed, self.ramp_start_speed) {
            (Some(current), Some(ramp_start)) => {
                let was_going_up = ramp_start < current;
                let now_going_up = current < speed;
                was_going_up != now_going_up // direction changed
            }
            _ => true,
        };

        if starting_new_ramp {
            self.ramp_start_speed = self.current_speed;
            self.ramp_start_time = Some(Instant::now());
        }

        self.current_speed = Some(speed);

        Ok(true)
    }

    fn read_actual_speed(&mut self, buf: &mut String) -> Result<u32> {
        buf.clear();
        self.input_file
            .read_to_string(buf)
            .map_err(Error::ActualSpeedRead)?;
        self.input_file.rewind().map_err(Error::ActualSpeedRead)?;
        buf.trim_end().parse().map_err(Error::ActualSpeedParse)
    }

    pub fn check_speed(&mut self, buf: &mut String) -> Result<()> {
        let Some(current_speed) = self.current_speed else {
            return Ok(());
        };

        let actual_speed = self.read_actual_speed(buf)?;
        // Check for stall immediately, regardless of settling period.
        // A stall is defined as actual speed being below the minimum safe
        // speed accounting for tolerance.
        #[allow(clippy::cast_precision_loss)]
        let stall_threshold =
            self.min_speed as f32 * (1.0 - self.config.speed_tolerance_percent / 100.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        if actual_speed < stall_threshold as u32 {
            log::error!(
                "Fan stall detected at {}! actual={actual_speed} RPM, \
                 min={} RPM, threshold={stall_threshold:.0} RPM",
                self.path.display(),
                self.min_speed,
            );
            return Ok(());
        }

        // Check if we're still within the settling period.
        if let (Some(ramp_start_time), Some(ramp_start_speed)) =
            (self.ramp_start_time, self.ramp_start_speed)
        {
            #[allow(clippy::cast_precision_loss)]
            let total_delta = current_speed.abs_diff(ramp_start_speed) as f32;
            #[allow(clippy::cast_precision_loss)]
            let settling_secs =
                self.config.settling_time_factor * total_delta / self.max_speed as f32;
            if ramp_start_time.elapsed() < Duration::from_secs_f32(settling_secs) {
                return Ok(());
            }
        }

        // Check if actual speed is within the tolerance band of the target.
        #[allow(clippy::cast_precision_loss)]
        let tolerance_rpm = current_speed as f32 * (self.config.speed_tolerance_percent / 100.0);
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let tolerance_rpm = tolerance_rpm as u32;
        let lower = current_speed.saturating_sub(tolerance_rpm);
        let upper = current_speed.saturating_add(tolerance_rpm);

        if actual_speed < lower || actual_speed > upper {
            log::warn!(
                "Fan speed out of tolerance at {}: target={current_speed} RPM, \
                 actual={actual_speed} RPM, \
                 tolerance=±{tolerance_rpm} RPM ({:.1}%)",
                self.path.display(),
                self.config.speed_tolerance_percent,
            );
        }

        Ok(())
    }

    pub fn calc_speed(&self, temp: u8) -> u32 {
        if self.config.always_full_speed {
            return self.max_speed;
        }

        if temp <= self.config.low_temp {
            return self.min_speed;
        }
        if temp >= self.config.high_temp {
            return self.max_speed;
        }

        let temp = u32::from(temp);
        let low_temp = u32::from(self.config.low_temp);
        let high_temp = u32::from(self.config.high_temp);
        match self.config.speed_curve {
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            SpeedCurve::Linear => {
                ((temp - low_temp) as f32 / (high_temp - low_temp) as f32
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            SpeedCurve::Exponential => {
                ((temp - low_temp).pow(3) as f32 / (high_temp - low_temp).pow(3) as f32
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
            #[allow(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                clippy::cast_precision_loss
            )]
            SpeedCurve::Logarithmic => {
                (((temp - low_temp) as f32).log((high_temp - low_temp) as f32)
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
        }
    }
}

impl Drop for FanController {
    fn drop(&mut self) {
        if let Err(e) = self.set_manual(false) {
            log::error!("Failed to reset fan to automatic on drop: {e}");
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum FanComponent {
    Min,
    Max,
    Input,
    Output,
    Manual,
}

impl FanComponent {
    pub fn to_path(self, base: &Path) -> PathBuf {
        fn add_suffix(base: &Path, suffix: &'static str) -> PathBuf {
            let file_name = base
                .file_name()
                .expect("Programmer error: path has no file name");
            base.with_file_name(format!("{}{suffix}", file_name.display()))
        }

        match self {
            FanComponent::Min => add_suffix(base, "_min"),
            FanComponent::Max => add_suffix(base, "_max"),
            FanComponent::Input => add_suffix(base, "_input"),
            FanComponent::Output => add_suffix(base, "_output"),
            FanComponent::Manual => add_suffix(base, "_manual"),
        }
    }
}

impl std::fmt::Display for FanComponent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Min => f.write_str("min speed"),
            Self::Max => f.write_str("max speed"),
            Self::Input => f.write_str("current/input speed"),
            Self::Output => f.write_str("target/output speed"),
            Self::Manual => f.write_str("manual control"),
        }
    }
}
