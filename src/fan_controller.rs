use std::{
    io::{Read, Seek, Write},
    path::PathBuf,
    time::Instant,
};

use crate::{
    config::{FanConfig, SpeedCurve},
    error::{Error, Result},
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
    manual_file: std::fs::File,
    output_file: std::fs::File,
    input_file: std::fs::File,
    config: FanConfig,
    current_speed: Option<u32>,
    previous_speed: Option<u32>,
    last_speed_change: Option<Instant>,

    min_speed: u32,
    max_speed: u32,
}

impl FanController {
    pub fn new(fan: PathBuf, config: FanConfig) -> Result<Self> {
        fn with_suffix(mut path: PathBuf, suffix: &str) -> Result<PathBuf> {
            let file_name = path
                .file_name()
                .and_then(|file_name| file_name.to_str())
                .ok_or(Error::FanPath)?;
            path.set_file_name(format!("{file_name}{suffix}"));
            Ok(path)
        }

        let min_speed = std::fs::read_to_string(with_suffix(fan.clone(), "_min")?)
            .map_err(Error::MinSpeedRead)?
            .trim()
            .parse()
            .map_err(Error::MinSpeedParse)?;

        let max_speed = std::fs::read_to_string(with_suffix(fan.clone(), "_max")?)
            .map_err(Error::MaxSpeedRead)?
            .trim_end()
            .parse()
            .map_err(Error::MaxSpeedParse)?;

        let mut open_options = std::fs::OpenOptions::new();
        open_options.write(true);

        let manual_file = open_options
            .open(with_suffix(fan.clone(), "_manual")?)
            .map_err(Error::FanOpen)?;

        let output_file = open_options
            .open(with_suffix(fan.clone(), "_output")?)
            .map_err(Error::FanOpen)?;

        let input_file = std::fs::OpenOptions::new()
            .read(true)
            .open(with_suffix(fan, "_input")?)
            .map_err(Error::FanOpen)?;

        let mut this = Self {
            manual_file,
            output_file,
            input_file,
            config,
            current_speed: None,
            previous_speed: None,
            last_speed_change: None,
            min_speed,
            max_speed,
        };
        log::info!("Found fan: {this:#?}");

        // Acquire manual control (see `Drop` impl)
        this.set_manual(true)?;

        Ok(this)
    }

    fn set_manual(&mut self, enabled: bool) -> Result<()> {
        write_trunc!(&mut self.manual_file, "{}", usize::from(enabled)).map_err(Error::FanWrite)?;
        Ok(())
    }

    pub fn set_speed(&mut self, mut speed: u32) -> Result<bool> {
        speed = speed.clamp(self.min_speed, self.max_speed);

        if self.current_speed == Some(speed) {
            return Ok(false);
        }

        write_trunc!(&mut self.output_file, "{speed}").map_err(Error::FanWrite)?;
        self.previous_speed = self.current_speed;
        self.current_speed = Some(speed);
        self.last_speed_change = Some(Instant::now());
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
        if let (Some(last_change), Some(prev_speed)) = (self.last_speed_change, self.previous_speed)
        {
            #[allow(clippy::cast_precision_loss)]
            let delta = current_speed.abs_diff(prev_speed) as f32;
            #[allow(clippy::cast_precision_loss)]
            let settling_secs = self.config.settling_time_factor * delta / self.max_speed as f32;
            let settling = std::time::Duration::from_secs_f32(settling_secs);
            if last_change.elapsed() < settling {
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
