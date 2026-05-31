use crate::{
    helper_args::BUNDLED_SUI_HELPER_ARG,
    output::{CliDiagnostic, CliStatus, CliStep},
    sui::{
        project::CliContext,
        runners::process::{command_step, run_peregrine_child},
    },
};
use peregrine_adapters::sui::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiCommandKind,
};
use serde_json::Value;
use std::{collections::BTreeMap, ffi::OsString, time::Instant};

pub fn run_build(context: &CliContext) -> CliStep {
    run_sui_step(context, "build", SuiCommandKind::MoveBuild)
}

pub fn run_test(context: &CliContext) -> CliStep {
    run_sui_step(context, "test", SuiCommandKind::MoveTest)
}

pub fn run_coverage(context: &CliContext) -> Vec<CliStep> {
    let coverage = run_sui_step(context, "coverage", SuiCommandKind::MoveCoverage);

    if coverage.status != CliStatus::Passed {
        return vec![coverage];
    }

    let summary = run_sui_step(
        context,
        "coverage-summary",
        SuiCommandKind::MoveCoverageSummary,
    );
    vec![coverage, summary]
}

fn run_sui_step(context: &CliContext, name: &str, kind: SuiCommandKind) -> CliStep {
    let started_at = Instant::now();
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = match adapter.package_command_for(kind, None, false) {
        Ok(command) => command,
        Err(error) => {
            return CliStep::failed(
                name,
                started_at,
                CliDiagnostic::error("sui-adapter", error.to_string()),
            );
        }
    };
    let args = std::iter::once(OsString::from(BUNDLED_SUI_HELPER_ARG))
        .chain(command.bundled_args_for_package(&context.package_root))
        .collect::<Vec<_>>();

    match run_peregrine_child(args) {
        Ok(output) => command_step(
            name,
            started_at,
            Some(command.display),
            output,
            BTreeMap::from([(
                "execution".to_string(),
                Value::String("bundled-sui".to_string()),
            )]),
        ),
        Err(error) => CliStep::failed(name, started_at, CliDiagnostic::error("sui", error)),
    }
}
