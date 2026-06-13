// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use peregrine_analysis::{
    AnalysisRequest, AnalysisStage, AnalysisTarget, ChainId, DynamicResultStatus,
};
use peregrine_lib::helper_args::{
    BUNDLED_SUI_HELPER_ARG, FORMAL_VERIFICATION_HELPER_ARG, MOVE_ANALYZER_HELPER_ARG,
    MOVY_FUZZ_HELPER_ARG,
};
use peregrine_sui_adapter::SuiAdapterSettings;
use peregrine_sui_project_loader::run_sui_analysis_blocking;
use serde_json::{Value, json};

fn main() {
    let mut args = std::env::args_os();
    let _binary = args.next();

    match args.next() {
        Some(arg) if arg.as_os_str() == std::ffi::OsStr::new(BUNDLED_SUI_HELPER_ARG) => {
            run_bundled_sui_helper(args);
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
        _ => peregrine_lib::run(),
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
    peregrine_sui_adapter::move_analyzer::run_bundled_move_analyzer_stdio();
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
