use crate::{
    helper_args::MOVY_FUZZ_HELPER_ARG,
    output::{CliDiagnostic, CliStep},
    sui::{
        args::FuzzArgs,
        project::CliContext,
        runners::process::{command_step, run_peregrine_child},
    },
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, ffi::OsString, time::Instant};

pub fn run_fuzz(context: &CliContext, args: &FuzzArgs) -> CliStep {
    let started_at = Instant::now();
    let command = format!(
        "peregrine {MOVY_FUZZ_HELPER_ARG} {} {} --time-limit-seconds {} --seed {}",
        context.project_root.display(),
        context.package_path,
        args.time_limit_seconds,
        args.seed,
    );
    let output = run_peregrine_child([
        OsString::from(MOVY_FUZZ_HELPER_ARG),
        context.project_root.as_os_str().to_os_string(),
        OsString::from(&context.package_path),
        OsString::from(args.time_limit_seconds.to_string()),
        OsString::from(args.seed.to_string()),
    ]);

    match output {
        Ok(output) => command_step(
            "fuzz",
            started_at,
            Some(command),
            output,
            BTreeMap::from([
                (
                    "engine".to_string(),
                    Value::String("movy-local-executor".to_string()),
                ),
                ("seed".to_string(), json!(args.seed)),
                (
                    "timeLimitSeconds".to_string(),
                    json!(args.time_limit_seconds),
                ),
            ]),
        ),
        Err(error) => CliStep::failed("fuzz", started_at, CliDiagnostic::error("fuzz", error)),
    }
}
