use std::{io::ErrorKind, num::NonZeroUsize, path::PathBuf, str::FromStr};

use nonempty::NonEmpty as NonEmptyVec;

use crate::{Error, Result, fan_controller::FanController};

#[cfg(debug_assertions)]
const CONFIG_FILE: &str = "./t2fand.conf";
#[cfg(not(debug_assertions))]
const CONFIG_FILE: &str = "/etc/t2fand.conf";

#[derive(Clone, Copy, Debug)]
pub enum SpeedCurve {
    Linear,
    Exponential,
    Logarithmic,
}

impl std::fmt::Display for SpeedCurve {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Linear => f.write_str("linear"),
            Self::Exponential => f.write_str("exponential"),
            Self::Logarithmic => f.write_str("logarithmic"),
        }
    }
}

impl FromStr for SpeedCurve {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "linear" => Self::Linear,
            "exponential" => Self::Exponential,
            "logarithmic" => Self::Logarithmic,
            _ => return Err(()),
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct FanConfig {
    pub low_temp: u8,
    pub high_temp: u8,
    pub speed_curve: SpeedCurve,
    pub always_full_speed: bool,
    pub speed_tolerance_percent: f32,
    pub settling_time_factor: f32,
}

impl FanConfig {
    fn write_property<'a, 'b: 'a>(
        self,
        setter: &'a mut ini::SectionSetter<'b>,
    ) -> &'a mut ini::SectionSetter<'b> {
        setter
            .set("low_temp", self.low_temp.to_string())
            .set("high_temp", self.high_temp.to_string())
            .set("speed_curve", self.speed_curve.to_string())
            .set("always_full_speed", self.always_full_speed.to_string())
            .set(
                "speed_tolerance_percent",
                self.speed_tolerance_percent.to_string(),
            )
            .set(
                "settling_time_factor",
                self.settling_time_factor.to_string(),
            )
    }

    fn validated(self) -> Result<Self> {
        if self.low_temp >= self.high_temp {
            return Err(Error::InvalidConfigRange(
                "low_temp must be less than high_temp",
            ));
        }
        if self.speed_tolerance_percent < 0.0 || self.speed_tolerance_percent > 100.0 {
            return Err(Error::InvalidConfigRange(
                "speed_tolerance_percent must be between 0 and 100",
            ));
        }
        if self.speed_tolerance_percent > 20.0 {
            log::warn!(
                "speed_tolerance_percent is {:.1}%, which is unusually high",
                self.speed_tolerance_percent
            );
        }
        if self.settling_time_factor < 0.0 || self.settling_time_factor > 60.0 {
            return Err(Error::InvalidConfigRange(
                "settling_time_factor must be between 0 and 60",
            ));
        }
        Ok(self)
    }
}

impl Default for FanConfig {
    fn default() -> Self {
        Self {
            low_temp: 55,
            high_temp: 75,
            speed_curve: SpeedCurve::Linear,
            always_full_speed: false,
            speed_tolerance_percent: 5.0,
            settling_time_factor: 5.0,
        }
    }
}

impl TryFrom<&ini::Properties> for FanConfig {
    type Error = Error;

    fn try_from(properties: &ini::Properties) -> Result<Self, Self::Error> {
        fn get_value<V: FromStr>(properties: &ini::Properties, key: &'static str) -> Result<V> {
            let value_str = properties.get(key).ok_or(Error::MissingConfigValue(key))?;
            value_str
                .parse()
                .map_err(|_| Error::InvalidConfigValue(key))
        }

        Self {
            low_temp: get_value(properties, "low_temp")?,
            high_temp: get_value(properties, "high_temp")?,
            speed_curve: get_value(properties, "speed_curve")?,
            always_full_speed: get_value(properties, "always_full_speed")?,
            speed_tolerance_percent: get_value(properties, "speed_tolerance_percent")?,
            settling_time_factor: get_value(properties, "settling_time_factor")?,
        }
        .validated()
    }
}

fn parse_config_file(file_raw: &str, fan_count: NonZeroUsize) -> Result<Vec<FanConfig>> {
    let file = ini::Ini::load_from_str(file_raw)?;
    let mut configs = Vec::with_capacity(fan_count.get());

    for i in 1..=fan_count.get() {
        let section = file
            .section(Some(format!("Fan{i}")))
            .ok_or(Error::MissingFanConfig(i))?;

        configs.push(FanConfig::try_from(section)?);
    }

    Ok(configs)
}

fn generate_config_file(fan_count: NonZeroUsize) -> Result<Vec<FanConfig>> {
    let mut config_file = ini::Ini::new();
    let mut configs = Vec::with_capacity(fan_count.get());
    for i in 1..=fan_count.get() {
        let config = FanConfig::default().validated()?;
        configs.push(config);

        let mut setter = config_file.with_section(Some(format!("Fan{i}")));
        config.write_property(&mut setter);
    }

    config_file
        .write_to_file(CONFIG_FILE)
        .map_err(Error::ConfigCreate)?;

    Ok(configs)
}

pub fn load_fan_configs(fans: NonEmptyVec<PathBuf>) -> Result<NonEmptyVec<FanController>> {
    let fan_count = fans.len_nonzero();
    let configs = match std::fs::read_to_string(CONFIG_FILE) {
        Ok(file_raw) => parse_config_file(&file_raw, fan_count)?,
        Err(err) if err.kind() == ErrorKind::NotFound => generate_config_file(fan_count)?,
        Err(err) => return Err(Error::ConfigRead(err)),
    };

    let fans = fans
        .into_iter()
        .zip(configs)
        .map(|(fan, config)| FanController::new(fan, config))
        .collect::<Result<_>>()?;

    Ok(NonEmptyVec::from_vec(fans).unwrap())
}
