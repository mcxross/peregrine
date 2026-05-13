// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const BUNDLED_SUI_HELPER_ARG: &str = "--peregrine-bundled-sui";
const MOVY_FUZZ_HELPER_ARG: &str = "--peregrine-movy-fuzz";

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

fn run_movy_fuzz_helper(mut args: impl Iterator<Item = std::ffi::OsString>) {
    let Some(root_path) = args.next() else {
        eprintln!("missing root path");
        std::process::exit(1);
    };
    let Some(package_path) = args.next() else {
        eprintln!("missing package path");
        std::process::exit(1);
    };

    let package_path = package_path.to_string_lossy().into_owned();
    match peregrine_movy_fuzz_adapter::run_movy_fuzz_blocking(
        std::path::PathBuf::from(root_path),
        &package_path,
        peregrine_movy_fuzz_adapter::MovyFuzzOptions::default(),
    ) {
        Ok(run) => {
            println!("{}", run.stdout);
            std::process::exit(0);
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
