use std::{io::Write, path::PathBuf};

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
    config: FanConfig,

    min_speed: u32,
    max_speed: u32,
}

impl FanController {
    pub fn new(fan: PathBuf, config: FanConfig) -> Result<Self> {
        fn join_suffix(mut path: PathBuf, suffix: &str) -> PathBuf {
            let file_name = path.file_name().unwrap().to_str().unwrap();
            path.set_file_name(format!("{file_name}{suffix}"));
            path
        }

        let min_speed = std::fs::read_to_string(join_suffix(fan.clone(), "_min"))
            .map_err(Error::MinSpeedRead)?
            .trim()
            .parse()
            .map_err(Error::MinSpeedParse)?;

        let max_speed = std::fs::read_to_string(join_suffix(fan.clone(), "_max"))
            .map_err(Error::MaxSpeedRead)?
            .trim_end()
            .parse()
            .map_err(Error::MaxSpeedParse)?;

        let mut open_options = std::fs::OpenOptions::new();
        open_options.write(true).truncate(true);

        let manual_file = open_options
            .open(join_suffix(fan.clone(), "_manual"))
            .map_err(Error::FanOpen)?;

        let output_file = open_options
            .open(join_suffix(fan, "_output"))
            .map_err(Error::FanOpen)?;

        let this = Self {
            manual_file,
            output_file,
            config,
            min_speed,
            max_speed,
        };

        log::info!("Found fan: {this:#?}");
        Ok(this)
    }

    pub fn set_manual(&mut self, enabled: bool) -> Result<()> {
        write_trunc!(&mut self.manual_file, "{}", usize::from(enabled)).map_err(Error::FanWrite)?;
        Ok(())
    }

    pub fn set_speed(&mut self, mut speed: u32) -> Result<()> {
        if speed < self.min_speed {
            speed = self.min_speed;
        } else if speed > self.max_speed {
            speed = self.max_speed;
        }

        log::info!("Setting fan speed to {speed}");
        write_trunc!(&mut self.output_file, "{speed}").map_err(Error::FanWrite)?;
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
            SpeedCurve::Linear => {
                ((temp - low_temp) as f32 / (high_temp - low_temp) as f32
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
            SpeedCurve::Exponential => {
                ((temp - low_temp).pow(3) as f32 / (high_temp - low_temp).pow(3) as f32
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
            SpeedCurve::Logarithmic => {
                (((temp - low_temp) as f32).log((high_temp - low_temp) as f32)
                    * (self.max_speed - self.min_speed) as f32) as u32
                    + self.min_speed
            }
        }
    }
}
