use crate::{
    helper_args::FORMAL_VERIFICATION_HELPER_ARG,
    output::{CliDiagnostic, CliStep},
    sui::{
        args::VerifyArgs,
        project::{CliContext, FormalTarget, formal_targets},
        runners::process::{command_step, run_peregrine_child},
    },
};
use peregrine_adapters::sui::{
    SuiAdapter, SuiAdapterEnvironment, SuiAdapterSettings, SuiFormalVerificationOptions,
};
use peregrine_dynamic_analysis::sui::formal_verification::{
    FormalVerificationOptions, formal_verification_manifest,
};
use serde_json::{Value, json};
use std::{collections::BTreeMap, ffi::OsString, time::Instant};

pub fn run_verify(context: &CliContext, args: &VerifyArgs) -> Vec<CliStep> {
    let targets = match formal_targets(context, args) {
        Ok(targets) => targets,
        Err(error) => return vec![CliStep::failed("verify", Instant::now(), error)],
    };

    targets
        .into_iter()
        .map(|target| run_verify_target(context, args, target))
        .collect()
}

fn run_verify_target(context: &CliContext, args: &VerifyArgs, target: FormalTarget) -> CliStep {
    let started_at = Instant::now();
    let options = FormalVerificationOptions {
        module_name: target.module_name.clone(),
        file_path: target.file_path.clone(),
        timeout_seconds: Some(args.timeout_seconds),
        verbose: true,
        trace: args.trace,
        keep_temp: args.keep_temp,
    };
    let manifest = match formal_verification_manifest(
        &context.project_root,
        &context.package_path,
        &options,
    ) {
        Ok(manifest) => manifest,
        Err(error) => {
            return CliStep::failed(
                format!("verify:{}", target.module_name),
                started_at,
                CliDiagnostic::error("verify", error.to_string()),
            );
        }
    };
    let adapter = SuiAdapter::new(SuiAdapterSettings::default(), SuiAdapterEnvironment::new());
    let command = adapter.formal_verification_command(&SuiFormalVerificationOptions {
        module_name: target.module_name.clone(),
        file_path: target.file_path.clone(),
        timeout_seconds: Some(args.timeout_seconds),
        verbose: true,
        trace: args.trace,
        keep_temp: args.keep_temp,
    });
    let output = run_peregrine_child([
        OsString::from(FORMAL_VERIFICATION_HELPER_ARG),
        context.project_root.as_os_str().to_os_string(),
        OsString::from(&context.package_path),
        OsString::from(&target.file_path),
        OsString::from(&target.module_name),
        OsString::from(args.timeout_seconds.to_string()),
    ]);

    match output {
        Ok(output) => command_step(
            format!("verify:{}", target.module_name),
            started_at,
            Some(command.display),
            output,
            BTreeMap::from([
                (
                    "packageRoot".to_string(),
                    Value::String(manifest.package_root.display().to_string()),
                ),
                ("file".to_string(), Value::String(manifest.file_path)),
                ("module".to_string(), Value::String(manifest.module_name)),
                (
                    "timeoutSeconds".to_string(),
                    json!(manifest.timeout_seconds),
                ),
                (
                    "execution".to_string(),
                    Value::String("bundled-sui-prover".to_string()),
                ),
            ]),
        ),
        Err(error) => CliStep::failed(
            format!("verify:{}", target.module_name),
            started_at,
            CliDiagnostic::error("verify", error),
        ),
    }
}
