use crate::{
    args::{Cli, CliCommand},
    output::{CliDiagnostic, CliReport, CliStep},
    sui::{
        args::AnalyzeArgs,
        project::{resolve_context, resolve_workspace_root},
        runners::{
            run_analyze, run_build, run_bytecode, run_call_graph, run_cfg, run_coverage, run_fuzz,
            run_import_package, run_new_package, run_object_graph, run_signatures, run_test,
            run_verify,
        },
    },
};
use std::time::Instant;

pub fn execute(cli: &Cli) -> CliReport {
    let started_at = Instant::now();
    let command_name = cli.command.name();

    if matches!(&cli.command, CliCommand::Bytecode(args) if args.interactive && cli.json) {
        return CliReport::usage_error(
            command_name,
            started_at,
            CliDiagnostic::error(
                "bytecode",
                "Interactive bytecode viewer cannot be combined with --json.",
            ),
        );
    }

    match &cli.command {
        CliCommand::ImportPackage(args) => {
            let workspace_root = match resolve_workspace_root(&cli.project) {
                Ok(workspace_root) => workspace_root,
                Err(error) => return CliReport::usage_error(command_name, started_at, error),
            };
            CliReport::from_steps(
                command_name,
                started_at,
                workspace_root.display().to_string(),
                ".",
                vec![run_import_package(&workspace_root, args)],
            )
        }
        CliCommand::NewPackage(args) => {
            let workspace_root = match resolve_workspace_root(&cli.project) {
                Ok(workspace_root) => workspace_root,
                Err(error) => return CliReport::usage_error(command_name, started_at, error),
            };
            CliReport::from_steps(
                command_name,
                started_at,
                workspace_root.display().to_string(),
                ".",
                vec![run_new_package(&workspace_root, args)],
            )
        }
        _ => {
            let context = match resolve_context(&cli.project, &cli.package) {
                Ok(context) => context,
                Err(error) => return CliReport::usage_error(command_name, started_at, error),
            };
            let steps = match &cli.command {
                CliCommand::Build => vec![run_build(&context)],
                CliCommand::Test => vec![run_test(&context)],
                CliCommand::Coverage => run_coverage(&context),
                CliCommand::Bytecode(args) => vec![run_bytecode(&context, args)],
                CliCommand::Signatures(args) => vec![run_signatures(&context, args)],
                CliCommand::CallGraph(args) => vec![run_call_graph(&context, args)],
                CliCommand::ObjectGraph(args) => vec![run_object_graph(&context, args)],
                CliCommand::Cfg(args) => vec![run_cfg(&context, args)],
                CliCommand::Fuzz(args) => vec![run_fuzz(&context, args)],
                CliCommand::Verify(args) => run_verify(&context, args),
                CliCommand::Analyze(args) => vec![run_analyze(&context, args)],
                CliCommand::CheckAll(args) => run_check_all(&context, args),
                CliCommand::ImportPackage(_) | CliCommand::NewPackage(_) => unreachable!(),
            };

            CliReport::from_steps(
                command_name,
                started_at,
                context.project_root.display().to_string(),
                context.package_path,
                steps,
            )
        }
    }
}

fn run_check_all(
    context: &crate::sui::project::CliContext,
    args: &crate::sui::args::CheckAllArgs,
) -> Vec<CliStep> {
    let mut steps = Vec::new();

    steps.push(run_build(context));
    steps.push(run_test(context));
    steps.extend(run_coverage(context));
    steps.push(run_analyze(
        context,
        &AnalyzeArgs {
            fail_on_findings: args.fail_on_findings,
            no_global_plugins: args.no_global_plugins,
            plugins: args.plugins.clone(),
            list_analyzers: false,
            rulesets: args.rulesets.clone(),
        },
    ));

    if args.skip_fuzz {
        steps.push(CliStep::skipped("fuzz", "Fuzzing skipped by --skip-fuzz."));
    } else {
        steps.push(run_fuzz(context, &args.fuzz_args()));
    }

    if args.skip_verify {
        steps.push(CliStep::skipped(
            "verify",
            "Formal verification skipped by --skip-verify.",
        ));
    } else {
        steps.extend(run_verify(context, &args.verify_args()));
    }

    steps
}

#[allow(dead_code)]
pub fn internal_error(command: &str, started_at: Instant, error: String) -> CliReport {
    CliReport::usage_error(command, started_at, CliDiagnostic::error("cli", error))
}
