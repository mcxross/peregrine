use crate::args;
use crate::helper_args;
use crate::output;
use crate::session;
use crate::workflow;
use clap::Parser;
use std::ffi::OsString;

pub fn is_helper_arg(arg: &OsString) -> bool {
    peregrine_helper_protocol::is_helper_mode_arg(arg)
}

pub fn run_cli_or_helper_from_args<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();

    match args.next() {
        Some(arg) if is_helper_arg(&arg) => run_external_helper(std::iter::once(arg).chain(args)),
        Some(arg) => run_cli_from_args(std::iter::once(arg).chain(args)),
        None => run_cli_from_args(std::iter::empty()),
    }
}

pub fn run_cli_from_args<I>(args: I) -> i32
where
    I: IntoIterator<Item = OsString>,
{
    let cli =
        match args::Cli::try_parse_from(std::iter::once(OsString::from("peregrine")).chain(args)) {
            Ok(cli) => cli,
            Err(error) => {
                let exit_code = error.exit_code();
                let _ = error.print();
                return exit_code;
            }
        };
    run_security_cli(cli)
}

pub fn run_security_cli(cli: args::Cli) -> i32 {
    let json = cli.json;
    let report = workflow::execute(&cli);
    let exit_code = report.exit_code;
    session::McpToolClient::shutdown_all();

    if let Err(error) = output::write_report(&report, json) {
        eprintln!("{error}");
        return output::EXIT_USAGE;
    }

    exit_code
}

pub fn run_external_helper(args: impl IntoIterator<Item = OsString>) -> i32 {
    let Some(executable) = helper_args::resolve_external_helper_executable() else {
        eprintln!(
            "Peregrine helper is unavailable. Install peregrine-helper beside the TUI binary or set PEREGRINE_HELPER."
        );
        return 1;
    };
    match std::process::Command::new(executable).args(args).status() {
        Ok(status) => status.code().unwrap_or(1),
        Err(error) => {
            eprintln!("Could not run Peregrine helper: {error}");
            1
        }
    }
}
