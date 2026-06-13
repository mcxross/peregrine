use super::{
    MoveAnalyzerAdapter, MoveAnalyzerAdapterEnvironment, MoveAnalyzerAdapterError,
    MoveAnalyzerAdapterSettings, MoveAnalyzerAdapterSource, MoveAnalyzerExecutionTarget,
};

#[test]
fn default_settings_prefer_bundled_library_source() {
    assert_eq!(
        MoveAnalyzerAdapterSettings::default().source,
        MoveAnalyzerAdapterSource::BundledLibrary
    );
}

#[test]
fn legacy_binary_source_settings_are_accepted() {
    let settings: MoveAnalyzerAdapterSettings =
        serde_json::from_str(r#"{"binarySource":"system"}"#).expect("settings");

    assert_eq!(settings.source, MoveAnalyzerAdapterSource::System);
}

#[test]
fn bundled_status_is_available() {
    let adapter = MoveAnalyzerAdapter::new(
        MoveAnalyzerAdapterSettings::default(),
        MoveAnalyzerAdapterEnvironment::new().with_common_user_locations(false),
    );
    let status = adapter.status();

    assert!(status.installed);
    assert_eq!(
        status.active_source,
        Some(MoveAnalyzerAdapterSource::BundledLibrary)
    );
}

#[test]
fn missing_system_binary_is_reported() {
    let adapter = MoveAnalyzerAdapter::new(
        MoveAnalyzerAdapterSettings {
            source: MoveAnalyzerAdapterSource::System,
            binary_path: None,
        },
        MoveAnalyzerAdapterEnvironment::new()
            .with_path(None)
            .with_common_user_locations(false),
    );

    assert_eq!(
        adapter.resolve(),
        Err(MoveAnalyzerAdapterError::MissingSystemBinary)
    );
    assert!(!adapter.status().installed);
}

#[test]
fn configured_binary_path_forces_system_source() {
    let adapter = MoveAnalyzerAdapter::new(
        MoveAnalyzerAdapterSettings {
            source: MoveAnalyzerAdapterSource::BundledLibrary,
            binary_path: Some("/tmp/move-analyzer".to_string()),
        },
        MoveAnalyzerAdapterEnvironment::new()
            .with_path(None)
            .with_common_user_locations(false),
    );

    assert_eq!(
        adapter.resolve(),
        Ok(MoveAnalyzerExecutionTarget::System {
            executable: "/tmp/move-analyzer".into()
        })
    );
    assert_eq!(
        adapter.status().preferred_source,
        MoveAnalyzerAdapterSource::System
    );
}
