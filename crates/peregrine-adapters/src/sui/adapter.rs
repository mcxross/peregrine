use super::{
    bundled, system, SuiAdapterEnvironment, SuiAdapterError, SuiAdapterSettings, SuiAdapterSource,
    SuiAdapterSourceStatus, SuiAdapterStatus, SuiCommandKind, SuiExecutionTarget,
    SuiFormalVerificationCommand, SuiFormalVerificationOptions, SuiMoveNewCommand,
    SuiPackageCommand,
};

pub struct SuiAdapter {
    settings: SuiAdapterSettings,
    environment: SuiAdapterEnvironment,
}

impl SuiAdapter {
    pub fn new(settings: SuiAdapterSettings, environment: SuiAdapterEnvironment) -> Self {
        Self {
            settings,
            environment,
        }
    }

    pub fn settings(&self) -> &SuiAdapterSettings {
        &self.settings
    }

    pub fn status(&self) -> SuiAdapterStatus {
        let bundled = bundled::status();
        let system = self.system_status();
        let preferred_source = if self.settings.configured_cli_path().is_some() {
            SuiAdapterSource::System
        } else {
            self.settings.source
        };
        let active = match preferred_source {
            SuiAdapterSource::Bundled if bundled.available => Some(&bundled),
            SuiAdapterSource::System if system.available => Some(&system),
            _ => None,
        };

        SuiAdapterStatus {
            installed: active.is_some(),
            version: active.and_then(|status| status.version.clone()),
            install_hint: active.is_none().then(|| self.install_hint()),
            active_source: active.map(|status| status.source),
            preferred_source,
            resolved_path: active.and_then(|status| status.path.clone()),
            bundled,
            system,
        }
    }

    pub fn resolve(&self) -> Result<SuiExecutionTarget, SuiAdapterError> {
        if let Some(path) = self.settings.configured_cli_path() {
            return Ok(SuiExecutionTarget::System {
                executable: path.into(),
            });
        }

        match self.settings.source {
            SuiAdapterSource::Bundled => Ok(SuiExecutionTarget::Bundled),
            SuiAdapterSource::System => {
                let executable = system::executable(&self.environment, None)
                    .ok_or(SuiAdapterError::MissingSystemBinary)?;

                Ok(SuiExecutionTarget::System { executable })
            }
        }
    }

    pub fn package_command(
        &self,
        command_kind: &str,
    ) -> Result<SuiPackageCommand, SuiAdapterError> {
        self.package_command_with_publish_options(command_kind, None, false)
    }

    pub fn package_command_with_build_env(
        &self,
        command_kind: &str,
        publish_build_env: Option<&str>,
    ) -> Result<SuiPackageCommand, SuiAdapterError> {
        self.package_command_with_publish_options(command_kind, publish_build_env, false)
    }

    pub fn package_command_with_publish_options(
        &self,
        command_kind: &str,
        publish_build_env: Option<&str>,
        with_unpublished_dependencies: bool,
    ) -> Result<SuiPackageCommand, SuiAdapterError> {
        self.package_command_for(
            SuiCommandKind::parse(command_kind)?,
            publish_build_env,
            with_unpublished_dependencies,
        )
    }

    pub fn package_command_for(
        &self,
        command_kind: SuiCommandKind,
        publish_build_env: Option<&str>,
        with_unpublished_dependencies: bool,
    ) -> Result<SuiPackageCommand, SuiAdapterError> {
        SuiPackageCommand::new(
            command_kind,
            self.resolve()?,
            publish_build_env,
            with_unpublished_dependencies,
        )
    }

    pub fn move_new_command(
        &self,
        project_name: &str,
    ) -> Result<SuiMoveNewCommand, SuiAdapterError> {
        SuiMoveNewCommand::new(project_name, self.resolve_move_new())
    }

    pub fn formal_verification_command(
        &self,
        options: &SuiFormalVerificationOptions,
    ) -> SuiFormalVerificationCommand {
        SuiFormalVerificationCommand::new(options)
    }

    fn system_status(&self) -> SuiAdapterSourceStatus {
        system::status(&self.environment, self.settings.configured_cli_path())
    }

    fn resolve_move_new(&self) -> SuiExecutionTarget {
        if let Some(path) = self.settings.configured_cli_path() {
            SuiExecutionTarget::System {
                executable: path.into(),
            }
        } else {
            SuiExecutionTarget::Bundled
        }
    }

    fn install_hint(&self) -> String {
        match self.settings.source {
            SuiAdapterSource::Bundled => {
                "Bundled Sui crate is linked from the app dependency and should be available."
                    .to_string()
            }
            SuiAdapterSource::System => {
                "User installed Sui CLI is selected but `sui` was not found on PATH.".to_string()
            }
        }
    }
}
