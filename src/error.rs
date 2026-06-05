use std::path::PathBuf;

use crate::fan_controller::FanComponent;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("T2 Fan Daemon must be run as root")]
    NotRoot,
    #[error("Fan not found")]
    NoFan,
    #[error("CPU temperature sensor not found")]
    NoCpu,

    #[error("Temperature sensor cannot be read")]
    TempRead(#[source] std::io::Error),
    #[error("Temperature sensor cannot be seeked")]
    TempSeek(#[source] std::io::Error),
    #[error("Temperature sensor cannot be parsed")]
    TempParse(#[source] std::num::ParseIntError),
    #[error("Temperature mean does not fit into u8")]
    TempCast(#[source] std::num::TryFromIntError),

    #[error("Cannot read actual fan speed")]
    ActualSpeedRead(#[source] std::io::Error),
    #[error("Cannot parse actual fan speed")]
    ActualSpeedParse(#[source] std::num::ParseIntError),

    #[error("Cannot write process lock file")]
    LockWrite(#[source] std::io::Error),
    #[error("T2 Fan Daemon is already running")]
    AlreadyRunning,

    #[error("{1}: {path}", path=.0.display())]
    Config(PathBuf, #[source] ConfigError),

    #[error("{1}: {path}", path=.1.component().map(|c| format!("{}", c.to_path(&.0).display())).unwrap_or_default())]
    Fan(PathBuf, #[source] FanError),

    #[error("Cannot setup shutdown signals")]
    Signal(#[source] std::io::Error),

    #[error("Programmer Error: Invalid glob pattern")]
    GlobPattern(
        #[from]
        #[source]
        glob::PatternError,
    ),
    #[error("Cannot read path from glob pattern")]
    Glob(
        #[from]
        #[source]
        glob::GlobError,
    ),
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("Cannot read config file")]
    Read(#[source] std::io::Error),
    #[error("Cannot create default config file")]
    Create(#[source] std::io::Error),
    #[error("Cannot parse config file")]
    Parse(
        #[from]
        #[source]
        ini::ParseError,
    ),
    #[error("Missing Fan{0} in config file")]
    MissingFan(usize),
    #[error("Missing {0} in config file")]
    MissingValue(&'static str),
    #[error("Invalid {0} in config file")]
    InvalidValue(&'static str),
    #[error("{0}")]
    InvalidRange(&'static str),
}

#[derive(Debug, thiserror::Error)]
pub enum FanError {
    #[error("Cannot complete search for fans")]
    Search(#[source] std::io::Error),
    #[error("Cannot open fan controller handle for {0}")]
    Open(FanComponent, #[source] std::io::Error),
    #[error("Cannot read fan controller handle for {0}")]
    Read(FanComponent, #[source] std::io::Error),
    #[error("Cannot write to fan controller for {0}")]
    Write(FanComponent, #[source] std::io::Error),
    #[error("Cannot parse fan controller output for {0}")]
    Parse(FanComponent, #[source] std::num::ParseIntError),
}

impl FanError {
    fn component(&self) -> Option<FanComponent> {
        match self {
            FanError::Open(c, _)
            | FanError::Read(c, _)
            | FanError::Write(c, _)
            | FanError::Parse(c, _) => Some(*c),
            FanError::Search(_) => None,
        }
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
