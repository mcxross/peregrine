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
fn publish_dry_run_uses_ephemeral_test_publish() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter
        .package_command_with_build_env("publish-dry-run", Some("testnet"))
        .expect("command");
    let pubfile_path = command
        .temp_pubfile_path
        .as_ref()
        .expect("temp pubfile")
        .display()
        .to_string();

    assert_eq!(
        command.args,
        [
            "client",
            "test-publish",
            "--dry-run",
            "--pubfile-path",
            pubfile_path.as_str(),
            "--build-env",
            "testnet",
            "."
        ]
    );
    assert_eq!(
        command.display,
        format!(
            "sui client test-publish --dry-run --pubfile-path {pubfile_path} --build-env testnet ."
        )
    );
}

#[test]
fn bundled_publish_dry_run_uses_active_build_env_and_temp_pubfile() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter
        .package_command_with_build_env("publish-dry-run", Some("testnet"))
        .expect("command");
    let pubfile_path = command
        .temp_pubfile_path
        .as_ref()
        .expect("temp pubfile")
        .display()
        .to_string();
    let args = command
        .bundled_args_for_package(Path::new("/tmp/package"))
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert_eq!(
        args,
        [
            "sui",
            "client",
            "test-publish",
            "--dry-run",
            "--pubfile-path",
            pubfile_path.as_str(),
            "--build-env",
            "testnet",
            "/tmp/package"
        ]
    );
}

#[test]
fn publish_dry_run_can_include_unpublished_dependencies() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter
        .package_command_with_publish_options("publish-dry-run", Some("testnet"), true)
        .expect("command");
    let pubfile_path = command
        .temp_pubfile_path
        .as_ref()
        .expect("temp pubfile")
        .display()
        .to_string();
    let args = command
        .bundled_args_for_package(Path::new("/tmp/package"))
        .into_iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    assert!(command.with_unpublished_dependencies);
    assert_eq!(
        command.args,
        [
            "client",
            "test-publish",
            "--dry-run",
            "--pubfile-path",
            pubfile_path.as_str(),
            "--build-env",
            "testnet",
            "--with-unpublished-dependencies",
            "."
        ]
    );
    assert_eq!(
        args,
        [
            "sui",
            "client",
            "test-publish",
            "--dry-run",
            "--pubfile-path",
            pubfile_path.as_str(),
            "--build-env",
            "testnet",
            "--with-unpublished-dependencies",
            "/tmp/package"
        ]
    );
}

#[test]
fn publish_dry_run_requires_active_build_env() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let error = adapter
        .package_command("publish-dry-run")
        .expect_err("error");

    assert_eq!(
        error,
        SuiAdapterError::CommandParse(
            "Active Sui environment is required for publish dry-runs.".to_string()
        )
    );
}

#[test]
fn publish_network_override_commands_are_not_supported() {
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let error = adapter
        .package_command("publish-devnet")
        .expect_err("error");

    assert_eq!(
        error,
        SuiAdapterError::UnsupportedCommand("publish-devnet".to_string())
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

#[cfg(unix)]
#[test]
fn system_move_build_execution_is_owned_by_the_adapter() {
    use std::{fs, os::unix::fs::PermissionsExt};

    let temp = tempfile::tempdir().expect("tempdir");
    let executable = temp.path().join("sui");
    fs::write(
        &executable,
        "#!/bin/sh\nprintf '%s|%s|%s|%s|%s|%s|%s|%s' \"$1\" \"$2\" \"$3\" \"$4\" \"$5\" \"$6\" \"$MOVE_HOME\" \"$SUI_CONFIG_DIR\"\n",
    )
    .expect("write executable");
    let mut permissions = fs::metadata(&executable).expect("metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(&executable, permissions).expect("permissions");

    let package_root = temp.path().join("package");
    fs::create_dir(&package_root).expect("package root");
    let output = run_system_sui_move_build_blocking(
        &executable,
        &package_root,
        &SuiMoveBuildOptions {
            default_move_flavor: Some("core".to_string()),
        },
    )
    .expect("move build");

    assert_eq!(
        output,
        SuiCommandOutput {
            status: Some(0),
            stdout: format!(
                "move|build|--path|{}|--default-move-flavor|core|{}|{}",
                package_root.display(),
                package_root.join(".move-home").display(),
                package_root.join(".sui-config").display(),
            ),
            stderr: String::new(),
        }
    );
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
