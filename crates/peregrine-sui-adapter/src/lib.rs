//! Sui execution boundary for Peregrine.

mod adapter;
mod analysis;
mod bundled;
mod command;
mod environment;
mod error;
mod formal;
mod keys;
mod settings;
mod status;
mod system;

pub use adapter::SuiAdapter;
pub use analysis::SuiChainAdapter;
pub use command::{
    SuiCommandKind, SuiCommandOutput, SuiExecutionTarget, SuiMoveNewCommand, SuiPackageCommand,
};
pub use environment::SuiAdapterEnvironment;
pub use error::SuiAdapterError;
pub use formal::{
    DEFAULT_FORMAL_VERIFICATION_TIMEOUT_SECONDS, SuiFormalVerificationCommand,
    SuiFormalVerificationOptions,
};
pub use keys::{
    SuiAddNetworkEnvRequest, SuiExportPrivateKeyRequest, SuiExportPrivateKeyResponse,
    SuiGenerateKeyRequest, SuiGenerateKeyResponse, SuiImportKeyRequest, SuiImportKeyResponse,
    SuiKeyAccount, SuiKeyConfigStatus, SuiKeyDiagnostic, SuiKeyDiagnosticLevel, SuiKeyManager,
    SuiKeyState, SuiNetworkEnv, SuiNetworkState, SuiRemoveKeyRequest, SuiRemoveNetworkEnvRequest,
    SuiRenameKeyAliasRequest, SuiSetActiveAddressRequest, SuiSetActiveNetworkEnvRequest,
};
pub use settings::{SuiAdapterSettings, SuiAdapterSource};
pub use status::{SuiAdapterSourceStatus, SuiAdapterStatus};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SuiMoveBuildOptions {
    pub default_move_flavor: Option<String>,
}

#[cfg(test)]
mod tests;

pub fn run_bundled_sui_blocking<I>(args: I) -> Result<SuiCommandOutput, SuiAdapterError>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    bundled::run_blocking(args.into_iter().collect())
}

pub fn run_system_sui_move_build_blocking(
    executable: &std::path::Path,
    package_root: &std::path::Path,
    options: &SuiMoveBuildOptions,
) -> Result<SuiCommandOutput, SuiAdapterError> {
    system::run_move_build_blocking(executable, package_root, options)
}
