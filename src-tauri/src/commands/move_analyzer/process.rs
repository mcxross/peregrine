use super::framing;
use crate::{
    helper_args::{MOVE_ANALYZER_HELPER_ARG, resolve_helper_executable},
    state::{MoveAnalyzerCommandState, MoveAnalyzerSession},
};
use peregrine_sui_adapter::move_analyzer::{
    MoveAnalyzerExecutionTarget, MoveAnalyzerServerCommand,
};
use serde::Serialize;
use serde_json::Value;
use std::{
    io::{BufRead, BufReader},
    path::Path,
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};
use tauri::{Emitter, Manager};

pub(crate) const MOVE_ANALYZER_MESSAGE_EVENT: &str = "move-analyzer-message";
pub(crate) const MOVE_ANALYZER_EXIT_EVENT: &str = "move-analyzer-exit";
pub(crate) const MOVE_ANALYZER_STDERR_EVENT: &str = "move-analyzer-stderr";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct MoveAnalyzerServerSession {
    pub(crate) session_id: String,
    pub(crate) root_path: String,
    pub(crate) command: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveAnalyzerMessageEvent {
    session_id: String,
    message: Value,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveAnalyzerExitEvent {
    session_id: String,
    status: Option<i32>,
    error: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MoveAnalyzerStderrEvent {
    session_id: String,
    chunk: String,
}

pub(crate) fn start_server(
    app: tauri::AppHandle,
    state: tauri::State<'_, MoveAnalyzerCommandState>,
    root_path: &str,
    command: MoveAnalyzerServerCommand,
) -> Result<MoveAnalyzerServerSession, String> {
    let root = Path::new(root_path)
        .canonicalize()
        .map_err(|error| format!("Could not read Move Analyzer root {root_path}: {error}"))?;

    if !root.is_dir() {
        return Err("Move Analyzer root path is not a directory.".to_string());
    }

    let root_path = root.to_string_lossy().into_owned();
    stop_sessions_for_root(&state, &root_path)?;

    let mut child = spawn_server_process(&root, &command)?;
    let session_id = format!("{}#{}", root_path, child.id());
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "Move Analyzer process did not expose stdin.".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "Move Analyzer process did not expose stdout.".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "Move Analyzer process did not expose stderr.".to_string())?;
    let child = Arc::new(Mutex::new(child));
    let stdin = Arc::new(Mutex::new(Box::new(stdin) as Box<dyn std::io::Write + Send>));
    let stderr_tail = Arc::new(Mutex::new(String::new()));

    {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| "Move Analyzer session state is poisoned.".to_string())?;
        sessions.insert(
            session_id.clone(),
            MoveAnalyzerSession {
                child: Arc::clone(&child),
                root_path: root_path.clone(),
                stdin: Arc::clone(&stdin),
            },
        );
    }

    spawn_stdout_reader(app.clone(), session_id.clone(), stdout);
    spawn_stderr_reader(
        app.clone(),
        session_id.clone(),
        stderr,
        Arc::clone(&stderr_tail),
    );
    spawn_exit_watcher(app, session_id.clone(), child, stderr_tail);

    Ok(MoveAnalyzerServerSession {
        session_id,
        root_path,
        command: command.display,
    })
}

pub(crate) fn send_message(
    state: &tauri::State<'_, MoveAnalyzerCommandState>,
    session_id: &str,
    message: Value,
) -> Result<(), String> {
    let stdin = {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| "Move Analyzer session state is poisoned.".to_string())?;
        sessions
            .get(session_id)
            .map(|session| Arc::clone(&session.stdin))
            .ok_or_else(|| "Move Analyzer session is not running.".to_string())?
    };
    let mut stdin = stdin
        .lock()
        .map_err(|_| "Move Analyzer session stdin is poisoned.".to_string())?;

    framing::write_message(stdin.as_mut(), &message)
        .map_err(|error| format!("Could not send Move Analyzer message: {error}"))
}

pub(crate) fn stop_session_by_id(
    state: &tauri::State<'_, MoveAnalyzerCommandState>,
    session_id: &str,
) -> Result<(), String> {
    let session = {
        let mut sessions = state
            .sessions
            .lock()
            .map_err(|_| "Move Analyzer session state is poisoned.".to_string())?;
        sessions.remove(session_id)
    };

    if let Some(session) = session {
        let mut child = session
            .child
            .lock()
            .map_err(|_| "Move Analyzer process state is poisoned.".to_string())?;
        if child
            .try_wait()
            .map_err(|error| error.to_string())?
            .is_none()
        {
            child
                .kill()
                .map_err(|error| format!("Could not stop Move Analyzer: {error}"))?;
        }
    }

    Ok(())
}

fn stop_sessions_for_root(
    state: &tauri::State<'_, MoveAnalyzerCommandState>,
    root_path: &str,
) -> Result<(), String> {
    let session_ids = {
        let sessions = state
            .sessions
            .lock()
            .map_err(|_| "Move Analyzer session state is poisoned.".to_string())?;

        sessions
            .iter()
            .filter_map(|(session_id, session)| {
                (session.root_path == root_path).then(|| session_id.clone())
            })
            .collect::<Vec<_>>()
    };

    for session_id in session_ids {
        stop_session_by_id(state, &session_id)?;
    }

    Ok(())
}

fn spawn_server_process(root: &Path, command: &MoveAnalyzerServerCommand) -> Result<Child, String> {
    let mut process = match &command.execution {
        MoveAnalyzerExecutionTarget::BundledLibrary => {
            let mut process = Command::new(resolve_helper_executable()?);
            process.arg(MOVE_ANALYZER_HELPER_ARG);
            process
        }
        MoveAnalyzerExecutionTarget::System { executable } => Command::new(executable),
    };

    process
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env("NO_COLOR", "1")
        .env("CLICOLOR", "0")
        .env("TERM", "dumb");

    process.spawn().map_err(|error| {
        format!(
            "Could not start Move Analyzer `{}` in {}: {error}",
            command.display,
            root.display()
        )
    })
}

fn spawn_stdout_reader(
    app: tauri::AppHandle,
    session_id: String,
    stdout: impl std::io::Read + Send + 'static,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stdout);

        loop {
            match framing::read_message(&mut reader) {
                Ok(Some(message)) => {
                    let _ = app.emit(
                        MOVE_ANALYZER_MESSAGE_EVENT,
                        MoveAnalyzerMessageEvent {
                            session_id: session_id.clone(),
                            message,
                        },
                    );
                }
                Ok(None) => break,
                Err(error) => {
                    let _ = app.emit(
                        MOVE_ANALYZER_EXIT_EVENT,
                        MoveAnalyzerExitEvent {
                            session_id: session_id.clone(),
                            status: None,
                            error: Some(format!("Could not read Move Analyzer message: {error}")),
                        },
                    );
                    break;
                }
            }
        }
    });
}

fn spawn_stderr_reader(
    app: tauri::AppHandle,
    session_id: String,
    stderr: impl std::io::Read + Send + 'static,
    stderr_tail: Arc<Mutex<String>>,
) {
    thread::spawn(move || {
        let mut reader = BufReader::new(stderr);
        let mut line = String::new();

        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => break,
                Ok(_) => {
                    append_stderr_tail(&stderr_tail, &line);
                    let _ = app.emit(
                        MOVE_ANALYZER_STDERR_EVENT,
                        MoveAnalyzerStderrEvent {
                            session_id: session_id.clone(),
                            chunk: line.clone(),
                        },
                    );
                }
                Err(error) => {
                    let chunk = format!("Could not read Move Analyzer stderr: {error}");
                    append_stderr_tail(&stderr_tail, &chunk);
                    let _ = app.emit(
                        MOVE_ANALYZER_STDERR_EVENT,
                        MoveAnalyzerStderrEvent {
                            session_id: session_id.clone(),
                            chunk,
                        },
                    );
                    break;
                }
            }
        }
    });
}

fn spawn_exit_watcher(
    app: tauri::AppHandle,
    session_id: String,
    child: Arc<Mutex<Child>>,
    stderr_tail: Arc<Mutex<String>>,
) {
    thread::spawn(move || {
        loop {
            let status = {
                let mut child = match child.lock() {
                    Ok(child) => child,
                    Err(_) => {
                        let _ = app.emit(
                            MOVE_ANALYZER_EXIT_EVENT,
                            MoveAnalyzerExitEvent {
                                session_id: session_id.clone(),
                                status: None,
                                error: Some("Move Analyzer process state is poisoned.".to_string()),
                            },
                        );
                        break;
                    }
                };

                match child.try_wait() {
                    Ok(Some(status)) => Some(Ok(status.code())),
                    Ok(None) => None,
                    Err(error) => Some(Err(error.to_string())),
                }
            };

            match status {
                Some(Ok(status)) => {
                    let state = app.state::<MoveAnalyzerCommandState>();
                    if let Ok(mut sessions) = state.sessions.lock() {
                        sessions.remove(&session_id);
                    }
                    let stderr = stderr_tail
                        .lock()
                        .ok()
                        .map(|tail| tail.trim().to_string())
                        .filter(|tail| !tail.is_empty());
                    let _ = app.emit(
                        MOVE_ANALYZER_EXIT_EVENT,
                        MoveAnalyzerExitEvent {
                            session_id: session_id.clone(),
                            status,
                            error: stderr,
                        },
                    );
                    break;
                }
                Some(Err(error)) => {
                    let _ = app.emit(
                        MOVE_ANALYZER_EXIT_EVENT,
                        MoveAnalyzerExitEvent {
                            session_id: session_id.clone(),
                            status: None,
                            error: Some(error),
                        },
                    );
                    break;
                }
                None => thread::sleep(Duration::from_millis(400)),
            }
        }
    });
}

fn append_stderr_tail(stderr_tail: &Arc<Mutex<String>>, chunk: &str) {
    const MAX_STDERR_TAIL: usize = 4000;

    if let Ok(mut tail) = stderr_tail.lock() {
        tail.push_str(chunk);
        if tail.len() > MAX_STDERR_TAIL {
            let excess = tail.len() - MAX_STDERR_TAIL;
            tail.drain(..excess);
        }
    }
}
