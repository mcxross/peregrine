use super::{run_agent, run_tui, AgentExit};
use crate::agent;
use crate::app::{self, ApplicationRuntime};
use crate::args;
use clap::Parser;
use crate::workbench::{App, WorkbenchExit};
use std::io;
use std::path::PathBuf;

pub fn run_mode_shell(
    root: Option<PathBuf>,
    initial_app_server: Option<agent::app_server_session::AppServerSession>,
) -> io::Result<i32> {
    let mut app = App::from_launch_dir(root)?;
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
                let agent_args = args::AgentArgs {
                    config_overrides: Default::default(),
                    inner: agent::Cli::parse_from(["peregrine"]),
                };
                match run_agent(
                    agent_args,
                    app.chat.active_thread_id(),
                    app_server,
                    shared_config,
                )? {
                    AgentExit::Quit(code) => return Ok(code),
                    AgentExit::SwitchToWorkbench {
                        thread_id,
                        app_server,
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
