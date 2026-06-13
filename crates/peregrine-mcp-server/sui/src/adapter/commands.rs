use crate::{
    artifacts::MovePackageContext,
    error::{SecurityToolsError, SecurityToolsResult},
};
use peregrine_helper_protocol::{BUNDLED_SUI_HELPER_ARG, resolve_external_helper_executable};
use peregrine_sui_adapter::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiCommandKind, SuiExecutionTarget,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecuritySuiCommandKind {
    Build,
    Test,
    Coverage,
    CoverageSummary,
    MoveFuzz,
    PublishDryRun,
}

impl SecuritySuiCommandKind {
    pub fn parse(value: &str) -> SecurityToolsResult<Self> {
        match value {
            "build" => Ok(Self::Build),
            "test" => Ok(Self::Test),
            "coverage" => Ok(Self::Coverage),
            "coverageSummary" => Ok(Self::CoverageSummary),
            "moveFuzz" => Ok(Self::MoveFuzz),
            "publishDryRun" => Ok(Self::PublishDryRun),
            // Keep this explicit so tests and model-facing errors prove that
            // real publish is not available through the security tool surface.
            "publish" => Err(SecurityToolsError::UnsupportedCommand(value.to_string())),
            _ => Err(SecurityToolsError::UnsupportedCommand(value.to_string())),
        }
    }

    fn to_adapter_kind(self) -> SuiCommandKind {
        match self {
            Self::Build => SuiCommandKind::MoveBuild,
            Self::Test => SuiCommandKind::MoveTest,
            Self::Coverage => SuiCommandKind::MoveCoverage,
            Self::CoverageSummary => SuiCommandKind::MoveCoverageSummary,
            Self::MoveFuzz => SuiCommandKind::MoveFuzz,
            Self::PublishDryRun => SuiCommandKind::PublishDryRun,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecurityCommand {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub display: String,
    pub execution: SecurityCommandExecution,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SecurityCommandExecution {
    BundledSui,
    SystemSui,
}

pub fn build_sui_package_command(
    ctx: &MovePackageContext,
    adapter_settings: &SuiAdapterSettings,
    kind: SecuritySuiCommandKind,
    publish_build_env: Option<&str>,
    with_unpublished_dependencies: bool,
) -> SecurityToolsResult<SecurityCommand> {
    let adapter = SuiAdapter::new(adapter_settings.clone(), SuiAdapterEnvironment::new());
    let command = adapter.package_command_for(
        kind.to_adapter_kind(),
        publish_build_env,
        with_unpublished_dependencies,
    )?;

    match &command.execution {
        SuiExecutionTarget::Bundled => {
            let helper = helper_executable()?;
            let mut argv = vec![
                helper.to_string_lossy().into_owned(),
                BUNDLED_SUI_HELPER_ARG.to_string(),
            ];
            argv.extend(
                command
                    .bundled_args_for_package(&ctx.package_root)
                    .into_iter()
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );
            Ok(SecurityCommand {
                command: argv,
                cwd: ctx.package_root.clone(),
                display: command.display,
                execution: SecurityCommandExecution::BundledSui,
            })
        }
        SuiExecutionTarget::System { executable } => {
            let mut argv = vec![executable.to_string_lossy().into_owned()];
            argv.extend(command.args);
            Ok(SecurityCommand {
                command: argv,
                cwd: ctx.package_root.clone(),
                display: command.display,
                execution: SecurityCommandExecution::SystemSui,
            })
        }
    }
}

pub fn build_sui_move_new_command(
    project_root: &Path,
    adapter_settings: &SuiAdapterSettings,
    package_name: &str,
) -> SecurityToolsResult<SecurityCommand> {
    let adapter = SuiAdapter::new(adapter_settings.clone(), SuiAdapterEnvironment::new());
    let command = adapter.move_new_command(package_name)?;

    match &command.execution {
        SuiExecutionTarget::Bundled => {
            let helper = helper_executable()?;
            let mut argv = vec![
                helper.to_string_lossy().into_owned(),
                BUNDLED_SUI_HELPER_ARG.to_string(),
            ];
            argv.extend(
                command
                    .bundled_args()
                    .into_iter()
                    .map(|arg| arg.to_string_lossy().into_owned()),
            );
            Ok(SecurityCommand {
                command: argv,
                cwd: project_root.to_path_buf(),
                display: command.display,
                execution: SecurityCommandExecution::BundledSui,
            })
        }
        SuiExecutionTarget::System { executable } => {
            let mut argv = vec![executable.to_string_lossy().into_owned()];
            argv.extend(command.args);
            Ok(SecurityCommand {
                command: argv,
                cwd: project_root.to_path_buf(),
                display: command.display,
                execution: SecurityCommandExecution::SystemSui,
            })
        }
    }
}

fn helper_executable() -> SecurityToolsResult<PathBuf> {
    resolve_external_helper_executable().ok_or_else(|| {
        SecurityToolsError::HelperExecutable(
            "Peregrine helper is unavailable; install peregrine-helper beside \
             peregrine-sui-mcp-server or set PEREGRINE_HELPER"
                .to_string(),
        )
    })
}

#[allow(dead_code)]
fn _assert_path_send_sync(_: &Path) {}

#[cfg(test)]
mod tests {
    use super::SecurityCommandExecution;
    use super::*;
    use peregrine_sui_adapter::SuiAdapterSource;

    #[test]
    fn command_rejects_publish() {
        assert!(matches!(
            SecuritySuiCommandKind::parse("publish"),
            Err(SecurityToolsError::UnsupportedCommand(command)) if command == "publish"
        ));
    }

    #[test]
    fn sui_command_uses_explicit_cli_path_through_adapter() {
        let temp = tempfile::tempdir().expect("tempdir");
        let package_root = temp.path().join("package");
        std::fs::create_dir_all(&package_root).expect("package dir");
        let ctx = MovePackageContext {
            project_root: temp.path().to_path_buf(),
            package_root: package_root.clone(),
            package_path: "package".to_string(),
            package_name: "sample".to_string(),
        };
        let settings = SuiAdapterSettings {
            source: SuiAdapterSource::Bundled,
            cli_path: Some("/opt/peregrine/bin/sui".to_string()),
        };

        let build =
            build_sui_package_command(&ctx, &settings, SecuritySuiCommandKind::Build, None, false)
                .expect("build command");
        assert_eq!(build.execution, SecurityCommandExecution::SystemSui);
        assert_eq!(build.cwd, package_root);
        assert_eq!(
            build.command,
            vec!["/opt/peregrine/bin/sui", "move", "build"]
        );

        let dry_run = build_sui_package_command(
            &ctx,
            &settings,
            SecuritySuiCommandKind::PublishDryRun,
            Some("testnet"),
            true,
        )
        .expect("publish dry-run command");
        assert_eq!(dry_run.execution, SecurityCommandExecution::SystemSui);
        assert_eq!(dry_run.command[0], "/opt/peregrine/bin/sui");
        assert!(dry_run.command.contains(&"test-publish".to_string()));
        assert!(dry_run.command.contains(&"--dry-run".to_string()));
        assert!(dry_run.command.contains(&"--build-env".to_string()));
        assert!(dry_run.command.contains(&"testnet".to_string()));
        assert!(
            dry_run
                .command
                .contains(&"--with-unpublished-dependencies".to_string())
        );
        assert!(!dry_run.command.contains(&"publish".to_string()));
    }
}
