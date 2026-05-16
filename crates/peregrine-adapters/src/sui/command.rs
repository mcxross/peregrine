use super::{bundled, SuiAdapterError, SuiAdapterSource};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SuiExecutionTarget {
    Bundled,
    System { executable: PathBuf },
}

impl SuiExecutionTarget {
    pub fn source(&self) -> SuiAdapterSource {
        match self {
            Self::Bundled => SuiAdapterSource::Bundled,
            Self::System { .. } => SuiAdapterSource::System,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiPackageCommand {
    pub execution: SuiExecutionTarget,
    pub args: Vec<String>,
    pub display: String,
    pub temp_pubfile_path: Option<PathBuf>,
    pub kind: SuiCommandKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiMoveNewCommand {
    pub execution: SuiExecutionTarget,
    pub project_name: String,
    pub args: Vec<String>,
    pub display: String,
}

impl SuiMoveNewCommand {
    pub(crate) fn new(
        project_name: &str,
        execution: SuiExecutionTarget,
    ) -> Result<Self, SuiAdapterError> {
        let project_name = validated_move_project_name(project_name)?;
        let args = vec!["move".to_string(), "new".to_string(), project_name.clone()];

        Ok(Self {
            execution,
            display: format!("sui move new {project_name}"),
            project_name,
            args,
        })
    }

    pub fn source(&self) -> SuiAdapterSource {
        self.execution.source()
    }

    pub fn bundled_args(&self) -> Vec<OsString> {
        let mut args = vec![OsString::from("sui")];
        args.extend(self.args.iter().map(OsString::from));
        args
    }
}

fn validated_move_project_name(project_name: &str) -> Result<String, SuiAdapterError> {
    let project_name = project_name.trim();

    if project_name.is_empty() {
        return Err(SuiAdapterError::InvalidProjectName(
            "Project name cannot be empty.".to_string(),
        ));
    }

    if project_name.len() > 128 {
        return Err(SuiAdapterError::InvalidProjectName(
            "Project name is too long.".to_string(),
        ));
    }

    let mut characters = project_name.chars();
    let Some(first) = characters.next() else {
        return Err(SuiAdapterError::InvalidProjectName(
            "Project name cannot be empty.".to_string(),
        ));
    };

    if !(first == '_' || first.is_ascii_alphabetic()) {
        return Err(SuiAdapterError::InvalidProjectName(
            "Project name must start with a letter or underscore.".to_string(),
        ));
    }

    if !characters.all(|character| character == '_' || character.is_ascii_alphanumeric()) {
        return Err(SuiAdapterError::InvalidProjectName(
            "Project name can only contain letters, numbers, and underscores.".to_string(),
        ));
    }

    Ok(project_name.to_string())
}

impl SuiPackageCommand {
    pub(crate) fn new(kind: SuiCommandKind, execution: SuiExecutionTarget) -> Self {
        let (args, display, temp_pubfile_path) = command_parts(kind);

        Self {
            execution,
            args,
            display,
            temp_pubfile_path,
            kind,
        }
    }

    pub fn source(&self) -> SuiAdapterSource {
        self.execution.source()
    }

    pub fn system_executable(&self) -> Option<&Path> {
        match &self.execution {
            SuiExecutionTarget::System { executable } => Some(executable),
            SuiExecutionTarget::Bundled => None,
        }
    }

    pub fn bundled_args_for_package(&self, package_root: &Path) -> Vec<OsString> {
        bundled_args_for_package(self.kind, package_root)
    }

    pub fn run_bundled_blocking(
        &self,
        package_root: &Path,
    ) -> Result<SuiCommandOutput, SuiAdapterError> {
        if !matches!(self.execution, SuiExecutionTarget::Bundled) {
            return Err(SuiAdapterError::InvalidExecutionSource {
                expected: SuiAdapterSource::Bundled,
                actual: self.source(),
            });
        }

        bundled::run_blocking(self.bundled_args_for_package(package_root))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SuiCommandOutput {
    pub status: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuiCommandKind {
    MoveBuild,
    MoveTest,
    MoveCoverage,
    MoveFuzz,
    PublishDryRun(SuiNetwork),
    Publish(SuiNetwork),
}

impl SuiCommandKind {
    pub fn parse(command_kind: &str) -> Result<Self, SuiAdapterError> {
        match command_kind {
            "move-build" => Ok(Self::MoveBuild),
            "move-test" => Ok(Self::MoveTest),
            "move-coverage" => Ok(Self::MoveCoverage),
            "move-fuzz" => Ok(Self::MoveFuzz),
            "publish-dry-run-localnet" => Ok(Self::PublishDryRun(SuiNetwork::Localnet)),
            "publish-dry-run-devnet" => Ok(Self::PublishDryRun(SuiNetwork::Devnet)),
            "publish-dry-run-testnet" => Ok(Self::PublishDryRun(SuiNetwork::Testnet)),
            "publish-dry-run-mainnet" => Ok(Self::PublishDryRun(SuiNetwork::Mainnet)),
            "publish-localnet" => Ok(Self::Publish(SuiNetwork::Localnet)),
            "publish-devnet" => Ok(Self::Publish(SuiNetwork::Devnet)),
            "publish-testnet" => Ok(Self::Publish(SuiNetwork::Testnet)),
            "publish-mainnet" => Ok(Self::Publish(SuiNetwork::Mainnet)),
            _ => Err(SuiAdapterError::UnsupportedCommand(
                command_kind.to_string(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SuiNetwork {
    Localnet,
    Devnet,
    Testnet,
    Mainnet,
}

impl SuiNetwork {
    fn as_cli_arg(self) -> &'static str {
        match self {
            Self::Localnet => "localnet",
            Self::Devnet => "devnet",
            Self::Testnet => "testnet",
            Self::Mainnet => "mainnet",
        }
    }
}

fn command_parts(command_kind: SuiCommandKind) -> (Vec<String>, String, Option<PathBuf>) {
    match command_kind {
        SuiCommandKind::MoveBuild => (
            command_args(&["move", "build"]),
            "sui move build".to_string(),
            None,
        ),
        SuiCommandKind::MoveTest => (
            command_args(&["move", "test"]),
            "sui move test".to_string(),
            None,
        ),
        SuiCommandKind::MoveCoverage => (
            command_args(&["move", "test", "--coverage"]),
            "sui move test --coverage".to_string(),
            None,
        ),
        SuiCommandKind::MoveFuzz => (
            command_args(&["move", "test", "--rand-num-iters", "256"]),
            "sui move test --rand-num-iters 256".to_string(),
            None,
        ),
        SuiCommandKind::Publish(network) => {
            let environment = network.as_cli_arg();
            let args = command_args(&["client", "publish", "--client.env", environment, "."]);

            (
                args,
                format!("sui client publish --client.env {environment} ."),
                None,
            )
        }
        SuiCommandKind::PublishDryRun(network) => {
            let environment = network.as_cli_arg();
            let args = command_args(&[
                "client",
                "publish",
                "--dry-run",
                "--client.env",
                environment,
                ".",
            ]);

            (
                args,
                format!("sui client publish --dry-run --client.env {environment} ."),
                None,
            )
        }
    }
}

fn bundled_args_for_package(command_kind: SuiCommandKind, package_root: &Path) -> Vec<OsString> {
    let package_root = package_root.as_os_str();
    let mut args = vec![OsString::from("sui")];

    match command_kind {
        SuiCommandKind::MoveBuild => {
            push_os_args(&mut args, ["move", "--path"]);
            args.push(package_root.to_os_string());
            push_os_args(&mut args, ["build"]);
        }
        SuiCommandKind::MoveTest => {
            push_os_args(&mut args, ["move", "--path"]);
            args.push(package_root.to_os_string());
            push_os_args(&mut args, ["test"]);
        }
        SuiCommandKind::MoveCoverage => {
            push_os_args(&mut args, ["move", "--path"]);
            args.push(package_root.to_os_string());
            push_os_args(&mut args, ["test", "--coverage"]);
        }
        SuiCommandKind::MoveFuzz => {
            push_os_args(&mut args, ["move", "--path"]);
            args.push(package_root.to_os_string());
            push_os_args(&mut args, ["test", "--rand-num-iters", "256"]);
        }
        SuiCommandKind::Publish(network) => {
            push_os_args(
                &mut args,
                ["client", "publish", "--client.env", network.as_cli_arg()],
            );
            args.push(package_root.to_os_string());
        }
        SuiCommandKind::PublishDryRun(network) => {
            push_os_args(
                &mut args,
                [
                    "client",
                    "publish",
                    "--dry-run",
                    "--client.env",
                    network.as_cli_arg(),
                ],
            );
            args.push(package_root.to_os_string());
        }
    }

    args
}

fn command_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

fn push_os_args<const N: usize>(args: &mut Vec<OsString>, values: [&str; N]) {
    args.extend(values.into_iter().map(OsString::from));
}
