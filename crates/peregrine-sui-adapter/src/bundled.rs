use crate::{SuiAdapterError, SuiAdapterSource, SuiAdapterSourceStatus, SuiCommandOutput};
use clap::Parser;
use std::ffi::OsString;

pub(crate) fn status() -> SuiAdapterSourceStatus {
    SuiAdapterSourceStatus {
        source: SuiAdapterSource::Bundled,
        available: true,
        version: None,
        path: None,
        error: None,
    }
}

pub(crate) fn run_blocking(args: Vec<OsString>) -> Result<SuiCommandOutput, SuiAdapterError> {
    let command = sui::sui_commands::SuiCommand::try_parse_from(args)
        .map_err(|error| SuiAdapterError::CommandParse(error.to_string()))?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|error| SuiAdapterError::Runtime(error.to_string()))?;

    match runtime.block_on(command.execute()) {
        Ok(()) => Ok(SuiCommandOutput {
            status: Some(0),
            stdout: String::new(),
            stderr: String::new(),
        }),
        Err(error) => Ok(SuiCommandOutput {
            status: Some(1),
            stdout: String::new(),
            stderr: format!("{error:#}"),
        }),
    }
}
