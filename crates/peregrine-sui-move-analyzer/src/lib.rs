//! Sui Move Analyzer execution boundary.
//!
//! `adapter` chooses the active execution target, `system` discovers user
//! installed binaries, and `bundled` exposes the linked Sui-flavored language
//! server entrypoint used by Peregrine helper processes.

mod adapter;
#[cfg(feature = "bundled")]
mod bundled;
mod command;
mod environment;
mod error;
mod settings;
mod status;
mod system;

#[cfg(test)]
mod tests;

pub use adapter::MoveAnalyzerAdapter;
#[cfg(feature = "bundled")]
pub use bundled::run_stdio as run_bundled_move_analyzer_stdio;
pub use command::{MoveAnalyzerExecutionTarget, MoveAnalyzerServerCommand};
pub use environment::MoveAnalyzerAdapterEnvironment;
pub use error::MoveAnalyzerAdapterError;
pub use settings::{MoveAnalyzerAdapterSettings, MoveAnalyzerAdapterSource};
pub use status::{MoveAnalyzerAdapterSourceStatus, MoveAnalyzerAdapterStatus};
