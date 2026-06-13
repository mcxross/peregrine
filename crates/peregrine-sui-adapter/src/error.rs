use super::SuiAdapterSource;
use std::fmt;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuiAdapterError {
    MissingSystemBinary,
    UnsupportedCommand(String),
    InvalidProjectName(String),
    CommandParse(String),
    CommandExecution(String),
    Runtime(String),
    InvalidExecutionSource {
        expected: SuiAdapterSource,
        actual: SuiAdapterSource,
    },
}

impl fmt::Display for SuiAdapterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingSystemBinary => write!(
                formatter,
                "User installed Sui CLI was not found on PATH. Install Sui CLI or switch to the bundled Sui CLI in Settings."
            ),
            Self::UnsupportedCommand(command_kind) => {
                write!(formatter, "Unsupported Sui command: {command_kind}")
            }
            Self::InvalidProjectName(message) => write!(formatter, "{message}"),
            Self::CommandParse(error) => {
                write!(formatter, "Could not parse bundled Sui command: {error}")
            }
            Self::CommandExecution(error) => {
                write!(formatter, "Could not execute system Sui command: {error}")
            }
            Self::Runtime(error) => {
                write!(formatter, "Could not start bundled Sui runtime: {error}")
            }
            Self::InvalidExecutionSource { expected, actual } => write!(
                formatter,
                "Cannot execute {actual:?} Sui command with the {expected:?} runner."
            ),
        }
    }
}

impl std::error::Error for SuiAdapterError {}
