use super::runtime::build_agent_runtime;
use crate::agent;
use crate::args;
use codex_arg0::Arg0DispatchPaths;
use peregrine_config::LoaderOverrides;
use std::io;

pub fn run_agent(
    mut agent_args: args::AgentArgs,
    resume_thread_id: Option<peregrine_types::ThreadId>,
    app_server: Option<agent::app_server_session::AppServerSession>,
    shared_config: Option<agent::legacy_core::config::Config>,
) -> io::Result<AgentExit> {
    let mut inner = agent_args.inner;
    inner.resume_session_id = resume_thread_id.map(|thread_id| thread_id.to_string());
    inner
        .config_overrides
        .raw_overrides
        .splice(0..0, agent_args.config_overrides.raw_overrides.drain(..));

    let runtime = build_agent_runtime()?;
    let result = runtime.block_on(agent::run_main_with_session(
        inner,
        agent_arg0_dispatch_paths()?,
        LoaderOverrides::default(),
        /*explicit_remote_endpoint*/ None,
        app_server,
        shared_config,
    ))?;

    match result.exit_info.exit_reason {
        agent::ExitReason::SwitchToWorkbench => Ok(AgentExit::SwitchToWorkbench {
            thread_id: result.exit_info.thread_id,
            app_server: result.app_server,
        }),
        agent::ExitReason::UserRequested => Ok(AgentExit::Quit(0)),
        agent::ExitReason::Fatal(message) => {
            eprintln!("ERROR: {message}");
            Ok(AgentExit::Quit(1))
        }
    }
}

pub(crate) fn agent_arg0_dispatch_paths() -> io::Result<Arg0DispatchPaths> {
    Ok(Arg0DispatchPaths {
        codex_self_exe: Some(std::env::current_exe()?),
        codex_linux_sandbox_exe: None,
        main_execve_wrapper_exe: None,
    })
}

pub enum AgentExit {
    Quit(i32),
    SwitchToWorkbench {
        thread_id: Option<peregrine_types::ThreadId>,
        app_server: Option<agent::app_server_session::AppServerSession>,
    },
}
