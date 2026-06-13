use super::{SuiAdapterError, SuiAdapterSource, bundled, system};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
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
    pub publish_build_env: Option<String>,
    pub with_unpublished_dependencies: bool,
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
    pub(crate) fn new(
        kind: SuiCommandKind,
        execution: SuiExecutionTarget,
        publish_build_env: Option<&str>,
        with_unpublished_dependencies: bool,
    ) -> Result<Self, SuiAdapterError> {
        let publish_build_env = normalized_publish_build_env(kind, publish_build_env)?;
        let temp_pubfile_path = if matches!(kind, SuiCommandKind::PublishDryRun) {
            Some(temp_pubfile_path())
        } else {
            None
        };
        let (args, display) = command_parts(
            kind,
            temp_pubfile_path.as_deref(),
            publish_build_env.as_deref(),
            with_unpublished_dependencies,
        );

        Ok(Self {
            execution,
            args,
            display,
            temp_pubfile_path,
            publish_build_env,
            with_unpublished_dependencies,
            kind,
        })
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
        bundled_args_for_package(
            self.kind,
            package_root,
            self.temp_pubfile_path.as_deref(),
            self.publish_build_env.as_deref(),
            self.with_unpublished_dependencies,
        )
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

    pub fn run_system_blocking(
        &self,
        package_root: &Path,
    ) -> Result<SuiCommandOutput, SuiAdapterError> {
        let SuiExecutionTarget::System { executable } = &self.execution else {
            return Err(SuiAdapterError::InvalidExecutionSource {
                expected: SuiAdapterSource::System,
                actual: self.source(),
            });
        };
        system::run_blocking(executable, &self.args, package_root)
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
    MoveCoverageSummary,
    MoveFuzz,
    PublishDryRun,
    Publish,
}

impl SuiCommandKind {
    pub fn parse(command_kind: &str) -> Result<Self, SuiAdapterError> {
        match command_kind {
            "move-build" => Ok(Self::MoveBuild),
            "move-test" => Ok(Self::MoveTest),
            "move-coverage" => Ok(Self::MoveCoverage),
            "move-coverage-summary" => Ok(Self::MoveCoverageSummary),
            "move-fuzz" => Ok(Self::MoveFuzz),
            "publish-dry-run" => Ok(Self::PublishDryRun),
            "publish" => Ok(Self::Publish),
            _ => Err(SuiAdapterError::UnsupportedCommand(
                command_kind.to_string(),
            )),
        }
    }
}

fn command_parts(
    command_kind: SuiCommandKind,
    temp_pubfile_path: Option<&Path>,
    publish_build_env: Option<&str>,
    with_unpublished_dependencies: bool,
) -> (Vec<String>, String) {
    match command_kind {
        SuiCommandKind::MoveBuild => (
            command_args(&["move", "build"]),
            "sui move build".to_string(),
        ),
        SuiCommandKind::MoveTest => (command_args(&["move", "test"]), "sui move test".to_string()),
        SuiCommandKind::MoveCoverage => (
            command_args(&["move", "test", "--coverage"]),
            "sui move test --coverage".to_string(),
        ),
        SuiCommandKind::MoveCoverageSummary => (
            command_args(&["move", "coverage", "summary"]),
            "sui move coverage summary".to_string(),
        ),
        SuiCommandKind::MoveFuzz => (
            command_args(&["move", "test", "--rand-num-iters", "256"]),
            "sui move test --rand-num-iters 256".to_string(),
        ),
        SuiCommandKind::Publish => publish_command_parts(
            false,
            temp_pubfile_path,
            publish_build_env,
            with_unpublished_dependencies,
        ),
        SuiCommandKind::PublishDryRun => publish_command_parts(
            true,
            temp_pubfile_path,
            publish_build_env,
            with_unpublished_dependencies,
        ),
    }
}

fn publish_command_parts(
    dry_run: bool,
    temp_pubfile_path: Option<&Path>,
    publish_build_env: Option<&str>,
    with_unpublished_dependencies: bool,
) -> (Vec<String>, String) {
    let mut args = vec!["client".to_string()];
    let mut display = "sui client".to_string();

    if dry_run {
        args.push("test-publish".to_string());
        display.push_str(" test-publish");
        args.push("--dry-run".to_string());
        display.push_str(" --dry-run");

        if let Some(pubfile_path) = temp_pubfile_path {
            let pubfile_path = pubfile_path.display().to_string();
            args.extend(["--pubfile-path".to_string(), pubfile_path.clone()]);
            display.push_str(&format!(" --pubfile-path {pubfile_path}"));
        }

        if let Some(build_env) = publish_build_env {
            args.extend(["--build-env".to_string(), build_env.to_string()]);
            display.push_str(&format!(" --build-env {build_env}"));
        }
    } else {
        args.push("publish".to_string());
        display.push_str(" publish");
    }

    if with_unpublished_dependencies {
        args.push("--with-unpublished-dependencies".to_string());
        display.push_str(" --with-unpublished-dependencies");
    }

    args.push(".".to_string());
    display.push_str(" .");

    (args, display)
}

fn bundled_args_for_package(
    command_kind: SuiCommandKind,
    package_root: &Path,
    temp_pubfile_path: Option<&Path>,
    publish_build_env: Option<&str>,
    with_unpublished_dependencies: bool,
) -> Vec<OsString> {
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
        SuiCommandKind::MoveCoverageSummary => {
            push_os_args(&mut args, ["move", "--path"]);
            args.push(package_root.to_os_string());
            push_os_args(&mut args, ["coverage", "summary"]);
        }
        SuiCommandKind::MoveFuzz => {
            push_os_args(&mut args, ["move", "--path"]);
            args.push(package_root.to_os_string());
            push_os_args(&mut args, ["test", "--rand-num-iters", "256"]);
        }
        SuiCommandKind::Publish => {
            push_publish_os_args(
                &mut args,
                false,
                package_root,
                temp_pubfile_path,
                publish_build_env,
                with_unpublished_dependencies,
            );
        }
        SuiCommandKind::PublishDryRun => {
            push_publish_os_args(
                &mut args,
                true,
                package_root,
                temp_pubfile_path,
                publish_build_env,
                with_unpublished_dependencies,
            );
        }
    }

    args
}

fn push_publish_os_args(
    args: &mut Vec<OsString>,
    dry_run: bool,
    package_root: &std::ffi::OsStr,
    temp_pubfile_path: Option<&Path>,
    publish_build_env: Option<&str>,
    with_unpublished_dependencies: bool,
) {
    args.push(OsString::from("client"));

    if dry_run {
        args.push(OsString::from("test-publish"));
        args.push(OsString::from("--dry-run"));

        if let Some(pubfile_path) = temp_pubfile_path {
            args.push(OsString::from("--pubfile-path"));
            args.push(pubfile_path.as_os_str().to_os_string());
        }

        if let Some(build_env) = publish_build_env {
            push_os_args(args, ["--build-env", build_env]);
        }
    } else {
        args.push(OsString::from("publish"));
    }

    if with_unpublished_dependencies {
        args.push(OsString::from("--with-unpublished-dependencies"));
    }

    args.push(package_root.to_os_string());
}

fn normalized_publish_build_env(
    kind: SuiCommandKind,
    publish_build_env: Option<&str>,
) -> Result<Option<String>, SuiAdapterError> {
    if !matches!(kind, SuiCommandKind::PublishDryRun) {
        return Ok(None);
    }

    let Some(build_env) = publish_build_env
        .map(str::trim)
        .filter(|env| !env.is_empty())
    else {
        return Err(SuiAdapterError::CommandParse(
            "Active Sui environment is required for publish dry-runs.".to_string(),
        ));
    };

    if build_env.chars().any(char::is_whitespace) {
        return Err(SuiAdapterError::CommandParse(
            "Sui environment aliases cannot contain whitespace.".to_string(),
        ));
    }

    Ok(Some(build_env.to_string()))
}

fn temp_pubfile_path() -> PathBuf {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    std::env::temp_dir().join(format!(
        "peregrine-sui-dry-run-{}-{timestamp}.toml",
        std::process::id()
    ))
}

fn command_args(args: &[&str]) -> Vec<String> {
    args.iter().map(|arg| (*arg).to_string()).collect()
}

fn push_os_args<const N: usize>(args: &mut Vec<OsString>, values: [&str; N]) {
    args.extend(values.into_iter().map(OsString::from));
}
