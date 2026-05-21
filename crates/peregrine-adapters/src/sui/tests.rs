use super::*;
use std::path::Path;

#[test]
fn default_settings_prefer_bundled_source() {
    assert_eq!(
        SuiAdapterSettings::default().source,
        SuiAdapterSource::Bundled
    );
}

#[test]
fn legacy_binary_source_settings_are_accepted() {
    let settings: SuiAdapterSettings =
        serde_json::from_str(r#"{"binarySource":"system"}"#).expect("settings");

    assert_eq!(settings.source, SuiAdapterSource::System);
}

#[test]
fn bundled_source_is_available_without_a_binary() {
    let adapter = SuiAdapter::new(
        SuiAdapterSettings::default(),
        SuiAdapterEnvironment::new()
            .with_path(None)
            .with_common_user_locations(false),
    );

    let status = adapter.status();

    assert!(status.installed);
    assert_eq!(status.active_source, Some(SuiAdapterSource::Bundled));
    assert!(status.bundled.available);
}

#[test]
fn move_build_command_uses_bundled_execution_by_default() {
    let adapter = SuiAdapter::new(
        SuiAdapterSettings::default(),
        SuiAdapterEnvironment::new()
            .with_path(None)
            .with_common_user_locations(false),
    );

    let command = adapter.package_command("move-build").expect("command");

    assert_eq!(command.execution, SuiExecutionTarget::Bundled);
    assert_eq!(command.args, ["move", "build"]);
    assert_eq!(command.display, "sui move build");
    assert_eq!(command.source(), SuiAdapterSource::Bundled);
}

#[test]
fn formal_verification_command_describes_bundled_prover_target() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let options = SuiFormalVerificationOptions::new("vault", "sources/vault.move");
    let command = adapter.formal_verification_command(&options);

    assert_eq!(command.module_name, "vault");
    assert_eq!(command.file_path, "sources/vault.move");
    assert_eq!(
        command.timeout_seconds,
        DEFAULT_FORMAL_VERIFICATION_TIMEOUT_SECONDS
    );
    assert_eq!(
        command.display,
        "bundled sui-prover --path <package> --modules vault --timeout 45"
    );
}

#[test]
fn bundled_move_args_pin_the_package_path() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter.package_command("move-coverage").expect("command");
    let args = command
        .bundled_args_for_package(Path::new("/tmp/package"))
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        args,
        [
            "sui",
            "move",
            "--path",
            "/tmp/package",
            "test",
            "--coverage"
        ]
    );
}

#[test]
fn coverage_summary_command_inspects_existing_coverage() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter
        .package_command("move-coverage-summary")
        .expect("command");
    let args = command
        .bundled_args_for_package(Path::new("/tmp/package"))
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(command.args, ["move", "coverage", "summary"]);
    assert_eq!(command.display, "sui move coverage summary");
    assert_eq!(
        args,
        [
            "sui",
            "move",
            "--path",
            "/tmp/package",
            "coverage",
            "summary"
        ]
    );
}

#[test]
fn publish_dry_run_command_does_not_use_pubfile_with_publish() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter
        .package_command("publish-dry-run-testnet")
        .expect("command");

    assert_eq!(
        command.args,
        [
            "client",
            "publish",
            "--dry-run",
            "--client.env",
            "testnet",
            "."
        ]
    );
    assert_eq!(command.temp_pubfile_path, None);
    assert_eq!(
        command.display,
        "sui client publish --dry-run --client.env testnet ."
    );
}

#[test]
fn system_source_reports_missing_when_path_is_empty() {
    let adapter = SuiAdapter::new(
        SuiAdapterSettings {
            source: SuiAdapterSource::System,
            cli_path: None,
        },
        SuiAdapterEnvironment::new()
            .with_path(None)
            .with_common_user_locations(false),
    );

    assert_eq!(adapter.resolve(), Err(SuiAdapterError::MissingSystemBinary));
    assert!(!adapter.status().installed);
}

#[test]
fn configured_cli_path_is_used_before_bundled_source() {
    let adapter = SuiAdapter::new(
        SuiAdapterSettings {
            source: SuiAdapterSource::Bundled,
            cli_path: Some("/opt/sui/bin/sui".to_string()),
        },
        SuiAdapterEnvironment::new()
            .with_path(None)
            .with_common_user_locations(false),
    );

    assert_eq!(
        adapter.resolve(),
        Ok(SuiExecutionTarget::System {
            executable: "/opt/sui/bin/sui".into(),
        })
    );
    assert_eq!(adapter.status().preferred_source, SuiAdapterSource::System);
}

#[test]
fn move_new_command_uses_cli_project_name() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter.move_new_command("vault").expect("command");

    assert_eq!(command.execution, SuiExecutionTarget::Bundled);
    assert_eq!(command.args, ["move", "new", "vault"]);
    assert_eq!(command.display, "sui move new vault");
    assert_eq!(
        command
            .bundled_args()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>(),
        ["sui", "move", "new", "vault"]
    );
}

#[test]
fn move_new_command_uses_bundled_when_system_source_has_no_cli_path() {
    let adapter = SuiAdapter::new(
        SuiAdapterSettings {
            source: SuiAdapterSource::System,
            cli_path: None,
        },
        SuiAdapterEnvironment::new(),
    );
    let command = adapter.move_new_command("vault").expect("command");

    assert_eq!(command.execution, SuiExecutionTarget::Bundled);
}

#[test]
fn move_new_command_uses_configured_cli_path() {
    let adapter = SuiAdapter::new(
        SuiAdapterSettings {
            source: SuiAdapterSource::Bundled,
            cli_path: Some("/opt/sui/bin/sui".to_string()),
        },
        SuiAdapterEnvironment::new(),
    );
    let command = adapter.move_new_command("vault").expect("command");

    assert_eq!(
        command.execution,
        SuiExecutionTarget::System {
            executable: "/opt/sui/bin/sui".into(),
        }
    );
}

#[test]
fn move_new_command_rejects_path_like_project_name() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());

    assert_eq!(
        adapter.move_new_command("../vault"),
        Err(SuiAdapterError::InvalidProjectName(
            "Project name must start with a letter or underscore.".to_string(),
        ))
    );
}

#[test]
fn unsupported_command_is_rejected() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());

    assert_eq!(
        adapter.package_command("unknown"),
        Err(SuiAdapterError::UnsupportedCommand("unknown".to_string()))
    );
}
