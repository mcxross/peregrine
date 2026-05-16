use super::{
    bundled, system, SuiAdapterEnvironment, SuiAdapterError, SuiAdapterSettings, SuiAdapterSource,
    SuiAdapterSourceStatus, SuiAdapterStatus, SuiCommandKind, SuiExecutionTarget,
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
        let preferred_source = self.settings.source;
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
        match self.settings.source {
            SuiAdapterSource::Bundled => Ok(SuiExecutionTarget::Bundled),
            SuiAdapterSource::System => {
                let executable = system::executable(&self.environment)
                    .ok_or(SuiAdapterError::MissingSystemBinary)?;

                Ok(SuiExecutionTarget::System { executable })
            }
        }
    }

    pub fn package_command(
        &self,
        command_kind: &str,
    ) -> Result<SuiPackageCommand, SuiAdapterError> {
        self.package_command_for(SuiCommandKind::parse(command_kind)?)
    }

    pub fn package_command_for(
        &self,
        command_kind: SuiCommandKind,
    ) -> Result<SuiPackageCommand, SuiAdapterError> {
        Ok(SuiPackageCommand::new(command_kind, self.resolve()?))
    }

    fn system_status(&self) -> SuiAdapterSourceStatus {
        system::status(&self.environment)
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
