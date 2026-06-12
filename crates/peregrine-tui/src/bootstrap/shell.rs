use super::{AgentExit, run_agent, run_tui};
use crate::agent;
use crate::app;
use crate::args;
use crate::workbench::{App, WorkbenchExit};
use clap::Parser;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::runtime::Runtime;

pub fn run_mode_shell(
    root: Option<PathBuf>,
    initial_app_server: Option<agent::app_server_session::AppServerSession>,
) -> io::Result<i32> {
    run_mode_shell_with_async_runtime(root, initial_app_server, None)
}

pub(crate) fn run_mode_shell_with_async_runtime(
    root: Option<PathBuf>,
    initial_app_server: Option<agent::app_server_session::AppServerSession>,
    async_runtime: Option<Arc<Runtime>>,
) -> io::Result<i32> {
    let mut app = App::from_launch_dir_with_async_runtime(root, async_runtime)?;
    if let (Some(runtime), Some(app_server)) = (&app.application_runtime, initial_app_server) {
        runtime.store_app_server(app_server);
    }
    loop {
        match run_tui(&mut app)? {
            WorkbenchExit::Quit => return Ok(0),
            WorkbenchExit::SwitchToAgent => {
                app.chat.suspend()?;
                let app_server = app
                    .application_runtime
                    .as_ref()
                    .and_then(app::ApplicationRuntime::take_app_server);
                let shared_config = app
                    .application_runtime
                    .as_ref()
                    .map(app::ApplicationRuntime::config)
                    .map(|config| config.as_ref().clone());
                let async_runtime = app
                    .application_runtime
                    .as_ref()
                    .map(app::ApplicationRuntime::async_runtime);
                let agent_args = args::AgentArgs {
                    config_overrides: Default::default(),
                    inner: agent::Cli::parse_from(["peregrine"]),
                };
                match run_agent(
                    agent_args,
                    app.chat.active_thread_id(),
                    app_server,
                    shared_config,
                    async_runtime,
                )? {
                    AgentExit::Quit(code) => return Ok(code),
                    AgentExit::SwitchToWorkbench {
                        thread_id,
                        app_server,
                        async_runtime: _,
                    } => {
                        if let (Some(runtime), Some(app_server)) =
                            (&app.application_runtime, app_server)
                        {
                            runtime.store_app_server(app_server);
                        }
                        if let Some(thread_id) = thread_id {
                            let root = app.explorer.root.clone();
                            app.chat.adopt_thread(&root, thread_id);
                        }
                    }
                }
            }
        }
    }
}
