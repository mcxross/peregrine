use semver::Version;

use crate::error::MemWalError;
use crate::types::RelayerVersionMetadata;

pub const SUPPORTED_RELAYER_API_MAJOR: u64 = 1;

pub fn assert_compatible_relayer(
    metadata: &RelayerVersionMetadata,
    server_url: &str,
) -> Result<(), MemWalError> {
    let api_version = Version::parse(&metadata.api_version)?;
    if api_version.major != SUPPORTED_RELAYER_API_MAJOR {
        return Err(MemWalError::compatibility(format!(
            "MemWal relayer at {server_url} reports unsupported apiVersion {}",
            metadata.api_version
        )));
    }

    if metadata.min_supported_sdk.typescript.trim().is_empty() {
        return Err(MemWalError::compatibility(format!(
            "MemWal relayer at {server_url} did not report minSupportedSdk.typescript"
        )));
    }

    Ok(())
}

pub fn compatibility_error_from_status(status: u16, body: &str) -> Option<MemWalError> {
    if status != 426 {
        return None;
    }

    Some(MemWalError::compatibility(format!(
        "MemWal relayer rejected this SDK as unsupported (HTTP 426). Relayer response: {}",
        body.chars().take(300).collect::<String>()
    )))
}
