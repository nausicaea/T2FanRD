use std::{
    fs::File,
    io::{Read, Seek, Write},
    path::PathBuf,
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
        fn with_suffix(mut path: PathBuf, suffix: &'static str) -> Result<PathBuf> {
            let file_name = path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .ok_or_else(|| Error::Fan(path.clone(), FanError::Path))?;
            path.set_file_name(format!("{file_name}{suffix}"));
            Ok(path)
        }

        fn open_with_suffix(path: PathBuf, suffix: &'static str, read_only: bool) -> Result<File> {
            let mut opt = std::fs::OpenOptions::new();
            if read_only {
                opt.read(true);
            } else {
                opt.write(true);
            }
            opt.open(with_suffix(path.clone(), suffix)?)
                .map_err(|e| Error::Fan(path, FanError::Open(suffix, e)))
        }

        let min_speed = open_with_suffix(fan.clone(), "_min", true).and_then(|mut fan_min| {
            temp_buf.clear();
            fan_min
                .read_to_string(temp_buf)
                .map_err(|e| Error::Fan(fan.clone(), FanError::Read("_min", e)))?;
            temp_buf
                .trim()
                .parse()
                .map_err(|e| Error::Fan(fan.clone(), FanError::Parse("_min", e)))
        })?;
        let max_speed = open_with_suffix(fan.clone(), "_max", true).and_then(|mut fan_max| {
            temp_buf.clear();
            fan_max
                .read_to_string(temp_buf)
                .map_err(|e| Error::Fan(fan.clone(), FanError::Read("_max", e)))?;
            temp_buf
                .trim()
                .parse()
                .map_err(|e| Error::Fan(fan.clone(), FanError::Parse("_max", e)))
        })?;

        let manual_file = open_with_suffix(fan.clone(), "_manual", false)?;
        let output_file = open_with_suffix(fan.clone(), "_output", false)?;
        let input_file = open_with_suffix(fan.clone(), "_input", true)?;

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
            &this.min_speed,
            &this.max_speed,
            &this.config,
        );

        // Acquire manual control (see `Drop` impl)
        this.set_manual(true).map_err(|e| Error::Fan(fan, e))?;

        Ok(this)
    }

    fn set_manual(&mut self, enabled: bool) -> Result<(), FanError> {
        write_trunc!(&mut self.manual_file, "{}", usize::from(enabled)).map_err(FanError::Write)?;
        Ok(())
    }

    pub fn set_speed(&mut self, mut speed: u32) -> Result<bool> {
        speed = speed.clamp(self.min_speed, self.max_speed);

        if self.current_speed == Some(speed) {
            return Ok(false);
        }

        write_trunc!(&mut self.output_file, "{speed}")
            .map_err(|e| Error::Fan(self.path.clone(), FanError::Write(e)))?;

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
                "Fan stall detected! actual={actual_speed} RPM, \
                 min={} RPM, threshold={stall_threshold:.0} RPM",
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
                "Fan speed out of tolerance: target={current_speed} RPM, \
                 actual={actual_speed} RPM, \
                 tolerance=±{tolerance_rpm} RPM ({:.1}%)",
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
