use std::path::PathBuf;

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

    #[error("Cannot read minimum fan speed")]
    MinSpeedRead(#[source] std::io::Error),
    #[error("Cannot parse minimum fan speed")]
    MinSpeedParse(#[source] std::num::ParseIntError),
    #[error("Cannot read maximum fan speed")]
    MaxSpeedRead(#[source] std::io::Error),
    #[error("Cannot parse maximum fan speed")]
    MaxSpeedParse(#[source] std::num::ParseIntError),
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

    #[error("{1}: {path}", path=.0.display())]
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
    #[error("The fan directory structure doesn't have the expected layout")]
    Path,
    #[error("Cannot open fan controller handle")]
    Open(#[source] std::io::Error),
    #[error("Cannot write to fan controller")]
    Write(#[source] std::io::Error),
    #[error("Cannot complete search for fans")]
    Search(#[source] std::io::Error),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
