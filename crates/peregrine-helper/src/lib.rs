use peregrine_analysis::{
    AnalysisRequest, AnalysisStage, AnalysisTarget, ChainId, DynamicResultStatus,
};
use peregrine_helper_protocol::{
    BUNDLED_SUI_HELPER_ARG, BYTECODE_VIEWER_HELPER_ARG, FORMAL_VERIFICATION_HELPER_ARG,
    HelperRequest, HelperResponse, JSON_PROTOCOL_HELPER_ARG, MOVE_ANALYZER_HELPER_ARG,
    MOVY_FUZZ_HELPER_ARG, parse_helper_request,
};
use peregrine_sui_adapter::SuiAdapterSettings;
use peregrine_sui_project_loader::run_sui_analysis_blocking;
use serde_json::{Value, json};
use std::io::{self, Read};

pub fn run() {
    let mut args = std::env::args_os();
    let _binary = args.next();

    match args.next() {
        Some(arg) if arg.as_os_str() == std::ffi::OsStr::new(JSON_PROTOCOL_HELPER_ARG) => {
            run_json_protocol();
        }
        Some(arg) if arg.as_os_str() == std::ffi::OsStr::new(BUNDLED_SUI_HELPER_ARG) => {
            run_bundled_sui_helper(args);
        }
        Some(arg) if arg.as_os_str() == std::ffi::OsStr::new(BYTECODE_VIEWER_HELPER_ARG) => {
            run_bytecode_viewer_helper(args);
        }
        Some(arg) if arg.as_os_str() == std::ffi::OsStr::new(MOVY_FUZZ_HELPER_ARG) => {
            run_movy_fuzz_helper(args);
        }
        Some(arg) if arg.as_os_str() == std::ffi::OsStr::new(FORMAL_VERIFICATION_HELPER_ARG) => {
            run_formal_verification_helper(args);
        }
        Some(arg) if arg.as_os_str() == std::ffi::OsStr::new(MOVE_ANALYZER_HELPER_ARG) => {
            run_move_analyzer_helper();
        }
        Some(arg) => {
            eprintln!("unknown Peregrine helper mode: {}", arg.to_string_lossy());
            std::process::exit(2);
        }
        None => {
            eprintln!("missing Peregrine helper mode");
            std::process::exit(2);
        }
    }
}

fn run_json_protocol() {
    let mut input = Vec::new();
    if let Err(error) = io::stdin().read_to_end(&mut input) {
        write_json_response(&HelperResponse::error(
            2,
            format!("Could not read helper request: {error}"),
        ));
        std::process::exit(2);
    }

    let response = handle_json_protocol(&input);
    let exit_code = response.status.unwrap_or(1);
    write_json_response(&response);
    std::process::exit(exit_code);
}

fn handle_json_protocol(input: &[u8]) -> HelperResponse {
    match parse_helper_request(input) {
        Ok(HelperRequest::Ping) => HelperResponse::ok(0, "pong\n", ""),
        Err(error) => HelperResponse::error(2, error),
    }
}

fn write_json_response(response: &HelperResponse) {
    match serde_json::to_writer(io::stdout(), response) {
        Ok(()) => println!(),
        Err(error) => {
            eprintln!("Could not encode helper response: {error}");
            std::process::exit(2);
        }
    }
}

fn run_bundled_sui_helper(args: impl IntoIterator<Item = std::ffi::OsString>) {
    match peregrine_sui_adapter::run_bundled_sui_blocking(args) {
        Ok(output) => {
            print!("{}", output.stdout);
            eprint!("{}", output.stderr);
            std::process::exit(output.status.unwrap_or(1));
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_move_analyzer_helper() {
    peregrine_sui_move_analyzer::run_bundled_move_analyzer_stdio();
}

fn run_bytecode_viewer_helper(mut args: impl Iterator<Item = std::ffi::OsString>) {
    let Some(package_root) = args.next() else {
        eprintln!("missing package root");
        std::process::exit(1);
    };
    let Some(module_name) = args.next() else {
        eprintln!("missing module name");
        std::process::exit(1);
    };
    let mut interactive = false;
    let mut bytecode_map = false;
    let mut debug = false;

    for arg in args {
        match arg.to_string_lossy().as_ref() {
            "--interactive" => interactive = true,
            "--bytecode-map" => bytecode_map = true,
            "--debug" => debug = true,
            unknown => {
                eprintln!("unknown bytecode viewer option: {unknown}");
                std::process::exit(1);
            }
        }
    }

    let package_root = std::path::PathBuf::from(package_root);
    let module_name = module_name.to_string_lossy().into_owned();
    let install_dir = tempfile::tempdir().expect("bytecode viewer install dir");
    let mut build_config = move_package_alt_compilation::build_config::BuildConfig::default();
    build_config.install_dir = Some(install_dir.path().to_path_buf());
    let disassemble = move_cli::base::disassemble::Disassemble {
        interactive,
        package_name: None,
        module_or_script_name: module_name,
        debug,
        bytecode_map,
    };
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()
        .expect("bytecode viewer runtime");
    let result = runtime.block_on(
        disassemble
            .execute::<sui_package_alt::SuiFlavor>(Some(package_root.as_path()), build_config),
    );
    if interactive {
        restore_bytecode_viewer_terminal();
    }

    match result {
        Ok(()) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
    }
}

fn restore_bytecode_viewer_terminal() {
    let mut stdout = std::io::stdout();
    let _ = crossterm::execute!(
        stdout,
        crossterm::event::DisableMouseCapture,
        crossterm::terminal::LeaveAlternateScreen
    );
    let _ = crossterm::terminal::disable_raw_mode();
}

fn run_movy_fuzz_helper(mut args: impl Iterator<Item = std::ffi::OsString>) {
    let Some(root_path) = args.next() else {
        eprintln!("missing root path");
        std::process::exit(1);
    };
    let Some(package_path) = args.next() else {
        eprintln!("missing package path");
        std::process::exit(1);
    };
    let time_limit_seconds = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(30);
    let seed = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(1);

    match run_dynamic_analysis(
        std::path::PathBuf::from(root_path),
        package_path.to_string_lossy().into_owned(),
        "fuzzing",
        json!({
            "timeLimitSeconds": time_limit_seconds,
            "seed": seed,
        }),
    ) {
        Ok(result) => {
            if let Some(stdout) = result.get("stdout").and_then(Value::as_str) {
                println!("{stdout}");
            }
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_formal_verification_helper(mut args: impl Iterator<Item = std::ffi::OsString>) {
    let Some(root_path) = args.next() else {
        eprintln!("missing root path");
        std::process::exit(1);
    };
    let Some(package_path) = args.next() else {
        eprintln!("missing package path");
        std::process::exit(1);
    };
    let Some(file_path) = args.next() else {
        eprintln!("missing file path");
        std::process::exit(1);
    };
    let Some(module_name) = args.next() else {
        eprintln!("missing module name");
        std::process::exit(1);
    };
    let timeout_seconds = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<usize>().ok());

    match run_dynamic_analysis(
        std::path::PathBuf::from(root_path),
        package_path.to_string_lossy().into_owned(),
        "formalVerification",
        json!({
            "filePath": file_path.to_string_lossy(),
            "moduleName": module_name.to_string_lossy(),
            "timeoutSeconds": timeout_seconds,
            "verbose": true,
            "trace": false,
            "keepTemp": false,
        }),
    ) {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

fn run_dynamic_analysis(
    project_root: std::path::PathBuf,
    package_path: String,
    capability: &str,
    options: Value,
) -> Result<Value, String> {
    let mut request = AnalysisRequest::safe(
        ChainId::new("sui"),
        AnalysisTarget::LocalPackage {
            path: project_root.clone(),
        },
    );
    request.stages = vec![AnalysisStage::Scan, AnalysisStage::Dynamic];
    request.graph_kinds.clear();
    request.dynamic_capabilities = vec![capability.to_string()];
    request
        .options
        .insert("projectRoot".to_string(), json!(project_root));
    request
        .options
        .insert("packagePath".to_string(), json!(package_path));
    if let Some(options) = options.as_object() {
        request.options.extend(
            options
                .iter()
                .map(|(key, value)| (key.clone(), value.clone())),
        );
    }

    let report = run_sui_analysis_blocking(request, SuiAdapterSettings::default())?;
    let result = report
        .dynamic_results
        .into_iter()
        .find(|result| result.capability == capability)
        .ok_or_else(|| format!("analysis did not return `{capability}` dynamic output"))?;
    if result.status == DynamicResultStatus::Completed {
        return Ok(result.result);
    }

    let message = result
        .diagnostics
        .iter()
        .chain(report.diagnostics.iter())
        .map(|diagnostic| diagnostic.message.as_str())
        .find(|message| !message.trim().is_empty())
        .unwrap_or("dynamic analysis failed");
    Err(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_protocol_ping_returns_success() {
        let response = handle_json_protocol(br#"{"kind":"ping"}"#);

        assert_eq!(response, HelperResponse::ok(0, "pong\n", ""));
    }

    #[test]
    fn json_protocol_invalid_request_returns_error_response() {
        let response = handle_json_protocol(b"{not-json");

        assert!(!response.ok);
        assert_eq!(response.status, Some(2));
        assert!(
            response
                .error
                .as_deref()
                .unwrap_or_default()
                .contains("Invalid helper request JSON")
        );
    }
}
