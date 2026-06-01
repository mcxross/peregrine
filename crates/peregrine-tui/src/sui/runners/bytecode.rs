use crate::{
    helper_args::BYTECODE_VIEWER_HELPER_ARG,
    output::{CliDiagnostic, CliStep},
    sui::{
        args::BytecodeArgs,
        project::{CliContext, bytecode_target},
        runners::process::{command_step, run_peregrine_child, run_peregrine_child_interactive},
    },
};
use serde_json::Value;
use std::{collections::BTreeMap, ffi::OsString, time::Instant};

pub fn run_bytecode(context: &CliContext, args: &BytecodeArgs) -> CliStep {
    let started_at = Instant::now();
    let target = match bytecode_target(context, args) {
        Ok(target) => target,
        Err(error) => return CliStep::failed("bytecode", started_at, error),
    };
    let helper_args = bytecode_helper_args(context, args, &target.module_name);
    let output = if args.interactive {
        run_peregrine_child_interactive(helper_args)
    } else {
        run_peregrine_child(helper_args)
    };

    match output {
        Ok(output) => command_step(
            "bytecode",
            started_at,
            Some(display_command(args, &target.module_name)),
            output,
            BTreeMap::from([
                (
                    "execution".to_string(),
                    Value::String("bundled-move-bytecode-viewer".to_string()),
                ),
                (
                    "compileMode".to_string(),
                    Value::String("compile-package-before-view".to_string()),
                ),
                ("module".to_string(), Value::String(target.module_name)),
                ("sourceFile".to_string(), Value::String(target.file_path)),
                ("interactive".to_string(), Value::Bool(args.interactive)),
            ]),
        ),
        Err(error) => CliStep::failed(
            "bytecode",
            started_at,
            CliDiagnostic::error("bytecode", error),
        ),
    }
}

fn bytecode_helper_args(
    context: &CliContext,
    args: &BytecodeArgs,
    module_name: &str,
) -> Vec<OsString> {
    let mut helper_args = vec![
        OsString::from(BYTECODE_VIEWER_HELPER_ARG),
        context.package_root.as_os_str().to_os_string(),
        OsString::from(module_name),
    ];

    if args.interactive {
        helper_args.push(OsString::from("--interactive"));
    }

    if args.bytecode_map {
        helper_args.push(OsString::from("--bytecode-map"));
    }

    if args.debug {
        helper_args.push(OsString::from("--debug"));
    }

    helper_args
}

fn display_command(args: &BytecodeArgs, module_name: &str) -> String {
    let mut command = format!("peregrine bytecode --module {module_name}");

    if args.interactive {
        command.push_str(" --interactive");
    }

    if args.bytecode_map {
        command.push_str(" --bytecode-map");
    }

    if args.debug {
        command.push_str(" --Xdebug");
    }

    command
}
