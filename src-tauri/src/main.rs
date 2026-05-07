// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

const BUNDLED_SUI_HELPER_ARG: &str = "--peregrine-bundled-sui";

fn main() {
    let mut args = std::env::args_os();
    let _binary = args.next();

    if args.next().as_deref() == Some(std::ffi::OsStr::new(BUNDLED_SUI_HELPER_ARG)) {
        run_bundled_sui_helper(args);
        return;
    }

    peregrine_lib::run()
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
