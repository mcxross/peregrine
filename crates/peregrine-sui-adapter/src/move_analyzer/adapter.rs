use super::{
    MoveAnalyzerAdapterEnvironment, MoveAnalyzerAdapterError, MoveAnalyzerAdapterSettings,
    MoveAnalyzerAdapterSource, MoveAnalyzerAdapterSourceStatus, MoveAnalyzerAdapterStatus,
    MoveAnalyzerExecutionTarget, MoveAnalyzerServerCommand, bundled, system,
};

pub struct MoveAnalyzerAdapter {
    settings: MoveAnalyzerAdapterSettings,
    environment: MoveAnalyzerAdapterEnvironment,
}

impl MoveAnalyzerAdapter {
    pub fn new(
        settings: MoveAnalyzerAdapterSettings,
        environment: MoveAnalyzerAdapterEnvironment,
    ) -> Self {
        Self {
            settings,
            environment,
        }
    }

    pub fn settings(&self) -> &MoveAnalyzerAdapterSettings {
        &self.settings
    }

    pub fn status(&self) -> MoveAnalyzerAdapterStatus {
        let bundled = bundled::status();
        let system = self.system_status();
        let preferred_source = if self.settings.configured_binary_path().is_some() {
            MoveAnalyzerAdapterSource::System
        } else {
            self.settings.source
        };
        let active = match preferred_source {
            MoveAnalyzerAdapterSource::BundledLibrary if bundled.available => Some(&bundled),
            MoveAnalyzerAdapterSource::System if system.available => Some(&system),
            _ => None,
        };

        MoveAnalyzerAdapterStatus {
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

    pub fn resolve(&self) -> Result<MoveAnalyzerExecutionTarget, MoveAnalyzerAdapterError> {
        if let Some(path) = self.settings.configured_binary_path() {
            return Ok(MoveAnalyzerExecutionTarget::System {
                executable: path.into(),
            });
        }

        match self.settings.source {
            MoveAnalyzerAdapterSource::BundledLibrary => {
                Ok(MoveAnalyzerExecutionTarget::BundledLibrary)
            }
            MoveAnalyzerAdapterSource::System => {
                let executable = system::executable(&self.environment, None)
                    .ok_or(MoveAnalyzerAdapterError::MissingSystemBinary)?;

                Ok(MoveAnalyzerExecutionTarget::System { executable })
            }
        }
    }

    pub fn server_command(&self) -> Result<MoveAnalyzerServerCommand, MoveAnalyzerAdapterError> {
        Ok(MoveAnalyzerServerCommand::new(self.resolve()?))
    }

    fn system_status(&self) -> MoveAnalyzerAdapterSourceStatus {
        system::status(&self.environment, self.settings.configured_binary_path())
    }

    fn install_hint(&self) -> String {
        match self.settings.source {
            MoveAnalyzerAdapterSource::BundledLibrary => {
                "Bundled Move Analyzer is linked from the app dependency and should be available."
                    .to_string()
            }
            MoveAnalyzerAdapterSource::System => {
                "User installed move-analyzer is selected but `move-analyzer` was not found on PATH."
                    .to_string()
            }
        }
    }
}
