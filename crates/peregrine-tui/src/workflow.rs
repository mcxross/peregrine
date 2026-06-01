use crate::{args::Cli, output::CliReport, sui};

pub fn execute(cli: &Cli) -> CliReport {
    sui::workflow::execute(cli)
}
