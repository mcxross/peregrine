use crate::helper_args::{
    BUNDLED_SUI_HELPER_ARG, FORMAL_VERIFICATION_HELPER_ARG, MOVY_FUZZ_HELPER_ARG,
    resolve_helper_executable,
};
use crate::{commands::files, validated_move_project_name};
use peregrine_sui_adapter::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiAdapterStatus,
    SuiAddNetworkEnvRequest, SuiExecutionTarget, SuiExportPrivateKeyRequest,
    SuiExportPrivateKeyResponse, SuiFormalVerificationCommand, SuiFormalVerificationOptions,
    SuiGenerateKeyRequest, SuiGenerateKeyResponse, SuiImportKeyRequest, SuiImportKeyResponse,
    SuiKeyManager, SuiKeyState, SuiMoveNewCommand, SuiNetworkState, SuiPackageCommand,
    SuiRemoveKeyRequest, SuiRemoveNetworkEnvRequest, SuiRenameKeyAliasRequest,
    SuiSetActiveAddressRequest, SuiSetActiveNetworkEnvRequest,
};
use peregrine_sui_dynamic_analysis::formal_verification::{
    FormalVerificationOptions, formal_verification_manifest,
};
use peregrine_sui_import_engine::{
    BuildVerification, BuildableImportRequest, ImportEngine, ImportEngineConfig,
    default_import_root,
};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tauri::{Emitter, Manager};

const SUI_ADAPTER_SETTINGS_CHANGED_EVENT: &str = "sui-adapter-settings-changed";
const SUI_ADAPTER_SETTINGS_FILE: &str = "sui-adapter-settings.json";
const SUI_COIN_TYPE: &str = "0x2::sui::SUI";
const SUI_GRAPHQL_BALANCE_QUERY: &str = r#"
query PeregrineSuiBalance($address: SuiAddress!, $coinType: String!) {
  address(address: $address) {
    balance(coinType: $coinType) {
      totalBalance
      coinBalance
      addressBalance
    }
  }
}
"#;

const COMMAND_OUTPUT_EVENT: &str = "command-output";
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CommandOutput {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuiWalletSummaryRequest {
    graph_ql_url: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuiWalletSummary {
    active_address: Option<String>,
    active_alias: Option<String>,
    balance: Option<SuiBalanceSummary>,
    balance_error: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SuiBalanceSummary {
    coin_type: String,
    total_balance_mist: String,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CommandOutputChunk {
    stream_id: String,
    stream: &'static str,
    chunk: String,
}

#[derive(Clone)]
struct CommandOutputStream {
    app: tauri::AppHandle,
    stream_id: String,
}

struct CommandReaderChunk {
    stream: &'static str,
    bytes: Vec<u8>,
}

#[tauri::command]
pub(crate) async fn create_move_project(
    app: tauri::AppHandle,
    parent_path: String,
    project_name: String,
    stream_id: Option<String>,
) -> Result<files::PackageTree, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sui = sui_adapter(&app)?;
        let command = sui
            .move_new_command(&project_name)
            .map_err(|error| error.to_string())?;
        let project_root = run_create_move_project_command(
            &parent_path,
            &project_name,
            command,
            command_output_stream(app, stream_id),
        )?;

        files::build_package_tree(
            project_root.to_string_lossy().into_owned(),
            files::PackageTreeMode::Shallow,
        )
    })
    .await
    .map_err(|error| format!("Could not join Move project creation task: {error}"))?
}

#[tauri::command]
pub(crate) async fn import_move_package_by_id(
    app: tauri::AppHandle,
    network_id: String,
    graph_ql_url: String,
    package_id: String,
    save_root_path: Option<String>,
    generate_buildable: bool,
) -> Result<files::PackageTree, String> {
    let import_root = if let Some(save_root_path) = save_root_path
        .as_deref()
        .map(str::trim)
        .filter(|path| !path.is_empty())
    {
        PathBuf::from(save_root_path)
    } else {
        let app_data_dir = app
            .path()
            .app_data_dir()
            .map_err(|error| format!("Could not resolve app data directory: {error}"))?;
        default_import_root(&app_data_dir, &network_id, &package_id)?
    };
    let engine = ImportEngine::new(ImportEngineConfig {
        max_dependency_depth: 3,
        max_dependency_packages: 64,
        build_verification: BuildVerification::SystemSui {
            executable: PathBuf::from("sui"),
            default_move_flavor: None,
        },
    });

    let imported_package = engine
        .import_buildable_package(BuildableImportRequest {
            import_root,
            network_id,
            graph_ql_url,
            package_id,
            generate_buildable,
        })
        .await?;

    tauri::async_runtime::spawn_blocking(move || {
        files::build_package_tree(
            imported_package.project_root.to_string_lossy().into_owned(),
            files::PackageTreeMode::Shallow,
        )
    })
    .await
    .map_err(|error| format!("Could not join imported package scan task: {error}"))?
}

#[tauri::command]
pub(crate) async fn build_move_package(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sui = sui_adapter(&app)?;
        run_package_command(
            &root_path,
            &package_path,
            sui.package_command("move-build")
                .map_err(|error| error.to_string())?,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join package build task: {error}"))?
}

#[tauri::command]
pub(crate) async fn run_security_command(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    command_kind: String,
    build_env: Option<String>,
    with_unpublished_dependencies: Option<bool>,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sui = sui_adapter(&app)?;
        let command = sui
            .package_command_with_publish_options(
                &command_kind,
                build_env.as_deref(),
                with_unpublished_dependencies.unwrap_or(false),
            )
            .map_err(|error| error.to_string())?;

        run_package_command(
            &root_path,
            &package_path,
            command,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join security command task: {error}"))?
}

#[tauri::command]
pub(crate) async fn run_movy_fuzz(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_movy_fuzz_worker(
            &root_path,
            &package_path,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join Movy fuzz task: {error}"))?
}

#[tauri::command]
pub(crate) async fn run_formal_verification(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    file_path: String,
    module_name: String,
    timeout_seconds: Option<usize>,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let sui = sui_adapter(&app)?;
        run_formal_verification_worker(
            &root_path,
            &package_path,
            sui.formal_verification_command(&SuiFormalVerificationOptions {
                file_path: file_path.clone(),
                module_name: module_name.clone(),
                timeout_seconds,
                verbose: true,
                trace: false,
                keep_temp: false,
            }),
            FormalVerificationOptions {
                file_path,
                module_name,
                timeout_seconds,
                verbose: true,
                trace: false,
                keep_temp: false,
            },
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join formal verification task: {error}"))?
}

fn run_movy_fuzz_worker(
    root_path: &str,
    package_path: &str,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let header = "Deploying package into Movy's local Sui executor and starting Movy fuzzing...\n";
    emit_command_output_chunk(stream.as_ref(), "stdout", header);

    let executable = resolve_helper_executable()?;
    let mut process = Command::new(executable);
    process
        .arg(MOVY_FUZZ_HELPER_ARG)
        .arg(root_path)
        .arg(package_path);

    let mut output = run_configured_command(configure_plain_command_output(&mut process), stream)?;
    output.stdout = format!("{header}{}", output.stdout);

    Ok(output)
}

fn run_formal_verification_worker(
    root_path: &str,
    package_path: &str,
    command: SuiFormalVerificationCommand,
    options: FormalVerificationOptions,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let manifest = formal_verification_manifest(root_path, package_path, &options)
        .map_err(|error| error.to_string())?;
    let header = format!(
        "Starting bundled Sui Prover formal verification...\nCommand: {}\nPackage: {}\nFile: {}\nModule filter: {}\n\n",
        command.display,
        manifest.package_root.display(),
        manifest.file_path,
        manifest.module_name,
    );
    emit_command_output_chunk(stream.as_ref(), "stdout", &header);

    let executable = resolve_helper_executable()?;
    let mut process = Command::new(executable);
    process
        .arg(FORMAL_VERIFICATION_HELPER_ARG)
        .arg(root_path)
        .arg(package_path)
        .arg(&options.file_path)
        .arg(&options.module_name)
        .arg(
            options
                .timeout_seconds
                .unwrap_or(command.timeout_seconds)
                .to_string(),
        );

    let mut output = run_configured_command(configure_plain_command_output(&mut process), stream)?;
    output.stdout = format!("{header}{}", output.stdout);

    Ok(output)
}

#[tauri::command]
pub(crate) async fn run_security_script(
    app: tauri::AppHandle,
    root_path: String,
    package_path: String,
    script_path: String,
    script_args: Vec<String>,
    stream_id: Option<String>,
) -> Result<CommandOutput, String> {
    tauri::async_runtime::spawn_blocking(move || {
        run_package_script(
            &root_path,
            &package_path,
            &script_path,
            &script_args,
            command_output_stream(app, stream_id),
        )
    })
    .await
    .map_err(|error| format!("Could not join security script task: {error}"))?
}

fn run_package_command(
    root_path: &str,
    package_path: &str,
    command: SuiPackageCommand,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let package_root = files::resolve_package_child_path(root_path, package_path)?;

    if !package_root.is_dir() {
        return Err("Selected package path is not a directory.".to_string());
    }

    if !package_root.join("Move.toml").is_file() {
        return Err("Selected package does not contain a Move.toml file.".to_string());
    }

    let output = match &command.execution {
        SuiExecutionTarget::Bundled => run_bundled_package_command(&command, &package_root, stream)
            .map_err(|error| {
                format!(
                    "Could not execute bundled `{}` for {}: {error}",
                    command.display,
                    package_root.display()
                )
            })?,
        SuiExecutionTarget::System { executable } => {
            let mut process = Command::new(executable);
            run_configured_command(
                configure_plain_command_output(
                    process.args(&command.args).current_dir(&package_root),
                ),
                stream,
            )
            .map_err(|error| {
                format!(
                    "Could not execute `{}` in {}: {error}",
                    command.display,
                    package_root.display()
                )
            })?
        }
    };
    cleanup_temp_pubfile(command.temp_pubfile_path.as_deref());

    Ok(output)
}

fn run_create_move_project_command(
    parent_path: &str,
    project_name: &str,
    command: SuiMoveNewCommand,
    stream: Option<CommandOutputStream>,
) -> Result<PathBuf, String> {
    let project_name = validated_move_project_name(project_name)?;
    let parent = PathBuf::from(parent_path)
        .canonicalize()
        .map_err(|error| format!("Could not read parent directory {parent_path}: {error}"))?;

    if !parent.is_dir() {
        return Err("Project parent path is not a directory.".to_string());
    }

    let project_root = parent.join(&project_name);

    if project_root.exists() {
        return Err(format!(
            "A file or folder named `{project_name}` already exists in {}.",
            parent.display()
        ));
    }

    let output = match &command.execution {
        SuiExecutionTarget::Bundled => run_bundled_move_new_command(&command, &parent, stream)
            .map_err(|error| {
                format!(
                    "Could not execute bundled `{}` in {}: {error}",
                    command.display,
                    parent.display()
                )
            })?,
        SuiExecutionTarget::System { executable } => {
            let mut process = Command::new(executable);
            run_configured_command(
                configure_plain_command_output(process.args(&command.args).current_dir(&parent)),
                stream,
            )
            .map_err(|error| {
                format!(
                    "Could not execute `{}` in {}: {error}",
                    command.display,
                    parent.display()
                )
            })?
        }
    };

    if output.status != Some(0) {
        let message = command_failure_message(&output);

        return Err(if message.is_empty() {
            format!(
                "`{}` failed in {} with status {:?}.",
                command.display,
                parent.display(),
                output.status
            )
        } else {
            format!(
                "`{}` failed in {} with status {:?}: {message}",
                command.display,
                parent.display(),
                output.status
            )
        });
    }

    let project_root = project_root.canonicalize().map_err(|error| {
        format!(
            "`{}` completed but Peregrine could not read {}: {error}",
            command.display,
            project_root.display()
        )
    })?;

    if !project_root.join("Move.toml").is_file() {
        return Err(format!(
            "`{}` completed but {} does not contain Move.toml.",
            command.display,
            project_root.display()
        ));
    }

    Ok(project_root)
}

fn run_bundled_package_command(
    command: &SuiPackageCommand,
    package_root: &Path,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let header = format!(
        "Running bundled Sui crate from the linked app dependency: {}\n",
        command.display
    );

    emit_command_output_chunk(stream.as_ref(), "stdout", &header);

    let executable = resolve_helper_executable()?;
    let mut process = Command::new(executable);
    process
        .arg(BUNDLED_SUI_HELPER_ARG)
        .args(command.bundled_args_for_package(package_root));

    let mut output = run_configured_command(&mut process, stream)?;
    output.stdout = format!("{header}{}", output.stdout);

    Ok(output)
}

fn run_bundled_move_new_command(
    command: &SuiMoveNewCommand,
    parent: &Path,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let header = format!(
        "Running bundled Sui crate from the linked app dependency: {}\n",
        command.display
    );

    emit_command_output_chunk(stream.as_ref(), "stdout", &header);

    let executable = resolve_helper_executable()?;
    let mut process = Command::new(executable);
    process
        .arg(BUNDLED_SUI_HELPER_ARG)
        .args(command.bundled_args())
        .current_dir(parent);

    let mut output = run_configured_command(&mut process, stream)?;
    output.stdout = format!("{header}{}", output.stdout);

    Ok(output)
}

fn command_failure_message(output: &CommandOutput) -> String {
    output
        .stderr
        .lines()
        .chain(output.stdout.lines())
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn emit_command_output_chunk(
    stream: Option<&CommandOutputStream>,
    stream_name: &'static str,
    chunk: impl AsRef<str>,
) {
    let chunk = chunk.as_ref();

    if chunk.is_empty() {
        return;
    }

    if let Some(stream) = stream {
        let _ = stream.app.emit(
            COMMAND_OUTPUT_EVENT,
            CommandOutputChunk {
                stream_id: stream.stream_id.clone(),
                stream: stream_name,
                chunk: chunk.to_string(),
            },
        );
    }
}

fn configure_plain_command_output(command: &mut Command) -> &mut Command {
    command
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .env("TERM", "dumb")
}

fn command_output_stream(
    app: tauri::AppHandle,
    stream_id: Option<String>,
) -> Option<CommandOutputStream> {
    let stream_id = stream_id?.trim().to_string();

    if stream_id.is_empty() {
        return None;
    }

    Some(CommandOutputStream { app, stream_id })
}

fn run_configured_command(
    command: &mut Command,
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| format!("Could not start process: {error}"))?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (sender, receiver) = mpsc::channel::<CommandReaderChunk>();

    if let Some(stdout) = stdout {
        spawn_output_reader(stdout, "stdout", sender.clone());
    }

    if let Some(stderr) = stderr {
        spawn_output_reader(stderr, "stderr", sender.clone());
    }

    drop(sender);

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();

    for chunk in receiver {
        match chunk.stream {
            "stdout" => stdout.extend_from_slice(&chunk.bytes),
            "stderr" => stderr.extend_from_slice(&chunk.bytes),
            _ => {}
        }

        if let Some(stream) = stream.as_ref() {
            let _ = stream.app.emit(
                COMMAND_OUTPUT_EVENT,
                CommandOutputChunk {
                    stream_id: stream.stream_id.clone(),
                    stream: chunk.stream,
                    chunk: String::from_utf8_lossy(&chunk.bytes).into_owned(),
                },
            );
        }
    }

    let status = child
        .wait()
        .map_err(|error| format!("Could not wait for process: {error}"))?;

    Ok(CommandOutput {
        status: status.code(),
        stdout: String::from_utf8_lossy(&stdout).into_owned(),
        stderr: String::from_utf8_lossy(&stderr).into_owned(),
    })
}

fn spawn_output_reader<R>(
    mut reader: R,
    stream: &'static str,
    sender: mpsc::Sender<CommandReaderChunk>,
) where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(size) => {
                    if sender
                        .send(CommandReaderChunk {
                            stream,
                            bytes: buffer[..size].to_vec(),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
}

fn cleanup_temp_pubfile(path: Option<&Path>) {
    if let Some(path) = path {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }
}

fn run_package_script(
    root_path: &str,
    package_path: &str,
    script_path: &str,
    script_args: &[String],
    stream: Option<CommandOutputStream>,
) -> Result<CommandOutput, String> {
    let package_root = files::resolve_package_child_path(root_path, package_path)?;
    let script_path = script_path.trim();

    if script_path.is_empty() {
        return Err("Bash script path cannot be empty.".to_string());
    }

    if script_path.len() > 1_024 {
        return Err("Bash script path is too long.".to_string());
    }

    if script_path.contains('\0') {
        return Err("Bash script path contains an invalid null byte.".to_string());
    }

    for script_arg in script_args {
        if script_arg.contains('\0') {
            return Err("Bash script argument contains an invalid null byte.".to_string());
        }

        if script_arg.len() > 1_024 {
            return Err("Bash script argument is too long.".to_string());
        }
    }

    if !package_root.is_dir() {
        return Err("Selected package path is not a directory.".to_string());
    }

    if !package_root.join("Move.toml").is_file() {
        return Err("Selected package does not contain a Move.toml file.".to_string());
    }

    let script_path = resolve_package_script_path(&package_root, script_path)?;
    let bundled_sui_shim_dir = create_bundled_sui_shim_dir()?;
    let bundled_sui_shim = bundled_sui_shim_dir.join("sui");
    let original_path = std::env::var_os("PATH").unwrap_or_default();
    let mut path_entries = vec![bundled_sui_shim_dir.clone()];
    path_entries.extend(std::env::split_paths(&original_path));
    let script_path_env = std::env::join_paths(path_entries)
        .map_err(|error| format!("Could not prepare script PATH: {error}"))?;

    let mut process = Command::new("bash");
    let output = run_configured_command(
        configure_plain_command_output(
            process
                .arg(&script_path)
                .args(script_args)
                .current_dir(&package_root)
                .env("PATH", script_path_env)
                .env("PEREGRINE_BUNDLED_SUI", &bundled_sui_shim)
                .env(
                    "PEREGRINE_COVERAGE_MAP_PATH",
                    package_root.join(".coverage_map.mvcov"),
                )
                .env("PEREGRINE_PACKAGE_ROOT", &package_root)
                .env("PEREGRINE_PROJECT_ROOT", root_path),
        ),
        stream,
    );
    let _ = fs::remove_dir_all(&bundled_sui_shim_dir);

    output.map_err(|error| {
        format!(
            "Could not execute bash script {} in {}: {error}",
            script_path.display(),
            package_root.display()
        )
    })
}

fn create_bundled_sui_shim_dir() -> Result<PathBuf, String> {
    let executable = resolve_helper_executable()?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let shim_dir = std::env::temp_dir().join(format!(
        "peregrine-bundled-sui-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir_all(&shim_dir).map_err(|error| {
        format!(
            "Could not create bundled Sui shim directory {}: {error}",
            shim_dir.display()
        )
    })?;

    let shim_path = shim_dir.join("sui");
    let script = format!(
        "#!/usr/bin/env bash\nexec {} {} sui \"$@\"\n",
        shell_quote(&executable.to_string_lossy()),
        shell_quote(BUNDLED_SUI_HELPER_ARG),
    );

    fs::write(&shim_path, script).map_err(|error| {
        format!(
            "Could not write bundled Sui shim {}: {error}",
            shim_path.display()
        )
    })?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(&shim_path)
            .map_err(|error| {
                format!(
                    "Could not read bundled Sui shim metadata {}: {error}",
                    shim_path.display()
                )
            })?
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&shim_path, permissions).map_err(|error| {
            format!(
                "Could not make bundled Sui shim executable {}: {error}",
                shim_path.display()
            )
        })?;
    }

    Ok(shim_dir)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn resolve_package_script_path(package_root: &Path, script_path: &str) -> Result<PathBuf, String> {
    let relative_script_path = Path::new(script_path);

    if relative_script_path.is_absolute() {
        return Err("Use a script path relative to the selected Move package.".to_string());
    }

    let script_path = package_root.join(relative_script_path);
    let script_path = script_path.canonicalize().map_err(|error| {
        format!(
            "Could not resolve bash script {}: {error}",
            script_path.display()
        )
    })?;

    if !script_path.starts_with(package_root) {
        return Err("Bash script must be inside the selected Move package.".to_string());
    }

    if !script_path.is_file() {
        return Err("Bash script path does not point to a file.".to_string());
    }

    Ok(script_path)
}

#[tauri::command]
pub(crate) async fn check_sui_adapter(app: tauri::AppHandle) -> Result<SuiAdapterStatus, String> {
    tauri::async_runtime::spawn_blocking(move || Ok(sui_adapter(&app)?.status()))
        .await
        .map_err(|error| format!("Could not join Sui adapter check task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_sui_key_state() -> Result<SuiKeyState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager.load_state().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui key state load task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_sui_network_state() -> Result<SuiNetworkState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager
            .load_network_state()
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui network state load task: {error}"))?
}

#[tauri::command]
pub(crate) async fn add_sui_network_env(
    request: SuiAddNetworkEnvRequest,
) -> Result<SuiNetworkState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager
            .add_network_env(request)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui network env add task: {error}"))?
}

#[tauri::command]
pub(crate) async fn set_active_sui_network_env(
    request: SuiSetActiveNetworkEnvRequest,
) -> Result<SuiNetworkState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager
            .set_active_network_env(request)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui active network env update task: {error}"))?
}

#[tauri::command]
pub(crate) async fn remove_sui_network_env(
    request: SuiRemoveNetworkEnvRequest,
) -> Result<SuiNetworkState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager
            .remove_network_env(request)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui network env remove task: {error}"))?
}

#[tauri::command]
pub(crate) async fn load_sui_wallet_summary(
    request: SuiWalletSummaryRequest,
) -> Result<SuiWalletSummary, String> {
    let state = tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager.load_state().map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui wallet summary load task: {error}"))??;

    let active_account = state
        .accounts
        .iter()
        .find(|account| account.is_active)
        .or_else(|| {
            state.active_address.as_ref().and_then(|address| {
                state
                    .accounts
                    .iter()
                    .find(|account| account.address == *address)
            })
        });
    let active_address = active_account
        .map(|account| account.address.clone())
        .or_else(|| state.active_address.clone());
    let active_alias = active_account.and_then(|account| account.alias.clone());

    let Some(address) = active_address.clone() else {
        return Ok(SuiWalletSummary {
            active_address: None,
            active_alias: None,
            balance: None,
            balance_error: None,
        });
    };

    let Some(graph_ql_url) = request
        .graph_ql_url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
    else {
        return Ok(SuiWalletSummary {
            active_address: Some(address),
            active_alias,
            balance: None,
            balance_error: Some("No GraphQL endpoint configured for this network.".to_string()),
        });
    };

    let balance = fetch_sui_balance(graph_ql_url, &address).await;
    let (balance, balance_error) = match balance {
        Ok(balance) => (Some(balance), None),
        Err(error) => (None, Some(error)),
    };

    Ok(SuiWalletSummary {
        active_address: Some(address),
        active_alias,
        balance,
        balance_error,
    })
}

#[tauri::command]
pub(crate) async fn generate_sui_key(
    request: SuiGenerateKeyRequest,
) -> Result<SuiGenerateKeyResponse, String> {
    let manager = sui_key_manager()?;

    manager
        .generate_key(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn import_sui_key(
    request: SuiImportKeyRequest,
) -> Result<SuiImportKeyResponse, String> {
    let manager = sui_key_manager()?;

    manager
        .import_key(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn rename_sui_key_alias(
    request: SuiRenameKeyAliasRequest,
) -> Result<SuiKeyState, String> {
    let manager = sui_key_manager()?;

    manager
        .rename_alias(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn set_active_sui_address(
    request: SuiSetActiveAddressRequest,
) -> Result<SuiKeyState, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager
            .set_active_address(request)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui active address update task: {error}"))?
}

#[tauri::command]
pub(crate) async fn remove_sui_key(request: SuiRemoveKeyRequest) -> Result<SuiKeyState, String> {
    let manager = sui_key_manager()?;

    manager
        .remove_key(request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn export_sui_private_key(
    request: SuiExportPrivateKeyRequest,
) -> Result<SuiExportPrivateKeyResponse, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let manager = sui_key_manager()?;
        manager
            .export_private_key(request)
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| format!("Could not join Sui private key export task: {error}"))?
}

#[tauri::command]
pub(crate) async fn get_sui_adapter_settings(
    app: tauri::AppHandle,
) -> Result<SuiAdapterSettings, String> {
    tauri::async_runtime::spawn_blocking(move || load_sui_adapter_settings(&app))
        .await
        .map_err(|error| format!("Could not join Sui adapter settings load task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_sui_adapter_settings(
    app: tauri::AppHandle,
    settings: SuiAdapterSettings,
) -> Result<SuiAdapterSettings, String> {
    tauri::async_runtime::spawn_blocking(move || {
        store_sui_adapter_settings(&app, &settings)?;
        let _ = app.emit(SUI_ADAPTER_SETTINGS_CHANGED_EVENT, &settings);

        Ok(settings)
    })
    .await
    .map_err(|error| format!("Could not join Sui adapter settings save task: {error}"))?
}

pub(crate) fn sui_adapter(app: &tauri::AppHandle) -> Result<SuiAdapter, String> {
    Ok(SuiAdapter::new(
        load_sui_adapter_settings(app)?,
        SuiAdapterEnvironment::new(),
    ))
}

fn sui_key_manager() -> Result<SuiKeyManager, String> {
    SuiKeyManager::new_default().map_err(|error| error.to_string())
}

async fn fetch_sui_balance(graph_ql_url: &str, address: &str) -> Result<SuiBalanceSummary, String> {
    let url = reqwest::Url::parse(graph_ql_url)
        .map_err(|error| format!("Invalid Sui GraphQL endpoint: {error}"))?;

    match url.scheme() {
        "http" | "https" => {}
        scheme => {
            return Err(format!(
                "Unsupported Sui GraphQL endpoint scheme `{scheme}`. Use http or https."
            ));
        }
    }

    let payload = serde_json::json!({
        "query": SUI_GRAPHQL_BALANCE_QUERY,
        "variables": {
            "address": address,
            "coinType": SUI_COIN_TYPE,
        },
    });
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| format!("Could not create Sui GraphQL client: {error}"))?;
    let response = client
        .post(url)
        .header(reqwest::header::CONTENT_TYPE, "application/json")
        .body(payload.to_string())
        .send()
        .await
        .map_err(|error| format!("Could not query Sui balance: {error}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|error| format!("Could not read Sui balance response: {error}"))?;

    if !status.is_success() {
        return Err(format!(
            "Sui balance query failed with HTTP status {status}."
        ));
    }

    let value: serde_json::Value = serde_json::from_str(&body)
        .map_err(|error| format!("Could not parse Sui balance response: {error}"))?;

    if let Some(message) = first_graphql_error_message(&value) {
        return Err(format!("Sui balance query failed: {message}"));
    }

    let total_balance_mist = value
        .pointer("/data/address/balance/totalBalance")
        .and_then(json_scalar_to_string)
        .ok_or_else(|| "Sui balance response did not include a total balance.".to_string())?;

    Ok(SuiBalanceSummary {
        coin_type: SUI_COIN_TYPE.to_string(),
        total_balance_mist,
    })
}

fn first_graphql_error_message(value: &serde_json::Value) -> Option<String> {
    value
        .get("errors")
        .and_then(serde_json::Value::as_array)
        .and_then(|errors| errors.first())
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
}

fn json_scalar_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(value) => Some(value.clone()),
        serde_json::Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn load_sui_adapter_settings(app: &tauri::AppHandle) -> Result<SuiAdapterSettings, String> {
    let path = sui_adapter_settings_path(app)?;

    if !path.is_file() {
        return Ok(SuiAdapterSettings::default());
    }

    let contents = fs::read_to_string(&path).map_err(|error| {
        format!(
            "Could not read Sui adapter settings {}: {error}",
            path.display()
        )
    })?;

    serde_json::from_str(&contents).map_err(|error| {
        format!(
            "Could not parse Sui adapter settings {}: {error}",
            path.display()
        )
    })
}

fn store_sui_adapter_settings(
    app: &tauri::AppHandle,
    settings: &SuiAdapterSettings,
) -> Result<(), String> {
    let path = sui_adapter_settings_path(app)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Could not create Sui adapter settings directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let contents = serde_json::to_string_pretty(settings)
        .map_err(|error| format!("Could not serialize Sui adapter settings: {error}"))?;

    fs::write(&path, format!("{contents}\n")).map_err(|error| {
        format!(
            "Could not write Sui adapter settings {}: {error}",
            path.display()
        )
    })
}

fn sui_adapter_settings_path(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app config directory: {error}"))?
        .join(SUI_ADAPTER_SETTINGS_FILE))
}
