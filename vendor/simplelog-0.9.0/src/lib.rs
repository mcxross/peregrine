use log::{LevelFilter, SetLoggerError};

#[derive(Clone, Debug, Default)]
pub struct Config;

#[derive(Clone, Debug, Default)]
pub struct ConfigBuilder {
    config: Config,
}

impl ConfigBuilder {
    pub fn new() -> Self {
        Self {
            config: Config,
        }
    }

    pub fn set_time_level(self, _level: LevelFilter) -> Self {
        self
    }

    pub fn set_level_padding(self, _padding: LevelPadding) -> Self {
        self
    }

    pub fn build(self) -> Config {
        self.config
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LevelPadding {
    Off,
}

#[derive(Clone, Copy, Debug)]
pub enum TerminalMode {
    Mixed,
}

#[derive(Clone, Debug)]
pub struct SimpleLogger {
    _level: LevelFilter,
    _config: Config,
}

impl SimpleLogger {
    pub fn new(level: LevelFilter, config: Config) -> Box<Self> {
        Box::new(Self {
            _level: level,
            _config: config,
        })
    }

    pub fn init(_level: LevelFilter, _config: Config) -> Result<(), SetLoggerError> {
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct TermLogger {
    _level: LevelFilter,
    _config: Config,
    _mode: TerminalMode,
}

impl TermLogger {
    pub fn new(level: LevelFilter, config: Config, mode: TerminalMode) -> Box<Self> {
        Box::new(Self {
            _level: level,
            _config: config,
            _mode: mode,
        })
    }
}

pub struct CombinedLogger;

impl CombinedLogger {
    pub fn init<T>(_loggers: Vec<Box<T>>) -> Result<(), SetLoggerError> {
        Ok(())
    }
}
