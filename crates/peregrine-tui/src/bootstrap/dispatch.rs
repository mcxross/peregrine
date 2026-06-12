use super::cli::{is_helper_arg, run_external_helper, run_security_cli};
use super::{run_agent, run_mode_shell, AgentExit};
use crate::args;
use clap::Parser;
use std::ffi::OsString;
use std::io;

pub fn run() -> io::Result<i32> {
    run_from_env_args(std::env::args_os())
}

pub fn run_from_env_args<I>(args: I) -> io::Result<i32>
where
    I: IntoIterator<Item = OsString>,
{
    let mut args = args.into_iter();
    let _binary = args.next();
    let args = args.collect::<Vec<_>>();

    if args.first().is_some_and(is_helper_arg) {
        return Ok(run_external_helper(args));
    }

    let cli = match args::ApplicationCli::try_parse_from(
        std::iter::once(OsString::from("peregrine")).chain(args),
    ) {
        Ok(cli) => cli,
        Err(error) => {
            let exit_code = error.exit_code();
            let _ = error.print();
            return Ok(exit_code);
        }
    };
    let args::ApplicationCli {
        workbench_root,
        project,
        package,
        json,
        command,
    } = cli;
    match command {
        None => run_mode_shell(workbench_root, None),
        Some(args::ApplicationCommand::Agent(agent_args)) => {
            match run_agent(agent_args, None, None, None)? {
                AgentExit::Quit(code) => Ok(code),
                AgentExit::SwitchToWorkbench { app_server, .. } => run_mode_shell(None, app_server),
            }
        }
        Some(args::ApplicationCommand::Security(command)) => Ok(run_security_cli(args::Cli {
            project,
            package,
            json,
            command,
        })),
    }
}
