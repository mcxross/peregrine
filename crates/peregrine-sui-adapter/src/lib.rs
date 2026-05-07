//! Sui execution boundary for Peregrine.
//!
//! `adapter` chooses the active execution target, `command` owns supported UI
//! command plans, `bundled` runs the linked Sui crate, and `system` resolves a
//! user-installed `sui` binary.

mod adapter;
mod bundled;
mod command;
mod environment;
mod error;
mod settings;
mod status;
mod system;

pub use adapter::SuiAdapter;
pub use command::{
    SuiCommandKind, SuiCommandOutput, SuiExecutionTarget, SuiNetwork, SuiPackageCommand,
};
pub use environment::SuiAdapterEnvironment;
pub use error::SuiAdapterError;
pub use settings::{SuiAdapterSettings, SuiAdapterSource};
pub use status::{SuiAdapterSourceStatus, SuiAdapterStatus};

#[cfg(test)]
mod tests;
