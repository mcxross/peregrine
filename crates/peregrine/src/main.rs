use peregrine_cli::helper_args::{
    BUNDLED_SUI_HELPER_ARG, FORMAL_VERIFICATION_HELPER_ARG, MOVY_FUZZ_HELPER_ARG,
};

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
        Some(arg) => {
            let exit_code = peregrine_cli::run_from_args(std::iter::once(arg).chain(args));
            std::process::exit(exit_code);
        }
        None => {
            let exit_code = peregrine_cli::run_from_args(std::iter::empty());
            std::process::exit(exit_code);
        }
    }
}

fn run_bundled_sui_helper(args: impl IntoIterator<Item = std::ffi::OsString>) {
    match peregrine_adapters::sui::run_bundled_sui_blocking(args) {
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
    let time_limit_seconds = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(30);
    let seed = args
        .next()
        .and_then(|value| value.to_string_lossy().parse::<u64>().ok())
        .unwrap_or(1);

    let package_path = package_path.to_string_lossy().into_owned();
    match peregrine_dynamic_analysis::sui::movy_fuzz::run_movy_fuzz_blocking(
        std::path::PathBuf::from(root_path),
        &package_path,
        peregrine_dynamic_analysis::sui::movy_fuzz::MovyFuzzOptions {
            time_limit_seconds,
            seed,
        },
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

    let package_path = package_path.to_string_lossy().into_owned();
    match peregrine_dynamic_analysis::sui::formal_verification::run_formal_verification_blocking(
        std::path::PathBuf::from(root_path),
        &package_path,
        peregrine_dynamic_analysis::sui::formal_verification::FormalVerificationOptions {
            file_path: file_path.to_string_lossy().into_owned(),
            module_name: module_name.to_string_lossy().into_owned(),
            timeout_seconds,
            verbose: true,
            trace: false,
            keep_temp: false,
        },
    ) {
        Ok(_) => std::process::exit(0),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
