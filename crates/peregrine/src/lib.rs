mod args;
pub mod helper_args;
mod output;
pub mod sui;
mod workflow;

use clap::Parser;
use std::ffi::OsString;

pub fn run_from_args<I>(args: I) -> i32
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
    let json = cli.json;
    let report = workflow::execute(&cli);
    let exit_code = report.exit_code;

    if let Err(error) = output::write_report(&report, json) {
        eprintln!("{error}");
        return output::EXIT_USAGE;
    }

    exit_code
}
