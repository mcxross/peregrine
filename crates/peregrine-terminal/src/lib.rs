use portable_pty::{ChildKiller, CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::{
    collections::HashMap,
    env,
    io::{Read, Write},
    path::PathBuf,
    sync::{Arc, Mutex},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[derive(Default)]
pub struct TerminalManager {
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
}

pub struct TerminalStartRequest {
    pub cwd: String,
    pub cols: u16,
    pub rows: u16,
}

pub struct TerminalStartResponse {
    pub session_id: String,
}

pub struct TerminalOutput {
    pub session_id: String,
    pub data: String,
}

pub struct TerminalExit {
    pub session_id: String,
    pub code: Option<i32>,
}

struct TerminalSession {
    child_killer: Arc<Mutex<Box<dyn ChildKiller + Send + Sync>>>,
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl TerminalManager {
    pub fn start(
        &self,
        request: TerminalStartRequest,
        on_output: impl Fn(TerminalOutput) + Send + 'static,
        on_exit: impl Fn(TerminalExit) + Send + 'static,
    ) -> Result<TerminalStartResponse, String> {
        let cwd = validated_cwd(&request.cwd)?;
        let size = terminal_size(request.cols, request.rows);
        let shell = default_shell();
        let session_id = new_session_id();

        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(size)
            .map_err(|error| format!("Could not open terminal PTY: {error}"))?;

        let mut command = CommandBuilder::new(&shell);
        command.cwd(cwd.as_os_str());
        command.env("TERM", "xterm-256color");
        command.env("COLORTERM", "truecolor");

        let child = pair
            .slave
            .spawn_command(command)
            .map_err(|error| format!("Could not start terminal shell `{shell}`: {error}"))?;

        drop(pair.slave);

        let child_killer = child.clone_killer();
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|error| format!("Could not attach terminal reader: {error}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|error| format!("Could not attach terminal writer: {error}"))?;

        {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| "Terminal session state is poisoned.".to_string())?;
            sessions.insert(
                session_id.clone(),
                TerminalSession {
                    child_killer: Arc::new(Mutex::new(child_killer)),
                    master: Arc::new(Mutex::new(pair.master)),
                    writer: Arc::new(Mutex::new(writer)),
                },
            );
        }

        spawn_terminal_reader(session_id.clone(), reader, on_output);
        spawn_terminal_exit_watcher(self.sessions.clone(), session_id.clone(), child, on_exit);

        Ok(TerminalStartResponse { session_id })
    }

    pub fn write(&self, session_id: &str, data: &str) -> Result<(), String> {
        let writer = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "Terminal session state is poisoned.".to_string())?;
            sessions
                .get(session_id)
                .map(|session| session.writer.clone())
                .ok_or_else(|| "Terminal session is no longer running.".to_string())?
        };
        let mut writer = writer
            .lock()
            .map_err(|_| "Terminal writer state is poisoned.".to_string())?;

        writer
            .write_all(data.as_bytes())
            .map_err(|error| format!("Could not write to terminal: {error}"))?;
        writer
            .flush()
            .map_err(|error| format!("Could not flush terminal input: {error}"))?;

        Ok(())
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let size = terminal_size(cols, rows);
        let master = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| "Terminal session state is poisoned.".to_string())?;
            sessions
                .get(session_id)
                .map(|session| session.master.clone())
                .ok_or_else(|| "Terminal session is no longer running.".to_string())?
        };
        let master = master
            .lock()
            .map_err(|_| "Terminal PTY state is poisoned.".to_string())?;

        master
            .resize(size)
            .map_err(|error| format!("Could not resize terminal: {error}"))?;

        Ok(())
    }

    pub fn stop(&self, session_id: &str) -> Result<(), String> {
        let session = {
            let mut sessions = self
                .sessions
                .lock()
                .map_err(|_| "Terminal session state is poisoned.".to_string())?;
            sessions.remove(session_id)
        };

        if let Some(session) = session {
            let mut child_killer = session
                .child_killer
                .lock()
                .map_err(|_| "Terminal process state is poisoned.".to_string())?;
            child_killer
                .kill()
                .map_err(|error| format!("Could not stop terminal: {error}"))?;
        }

        Ok(())
    }
}

fn spawn_terminal_reader(
    session_id: String,
    mut reader: Box<dyn Read + Send>,
    on_output: impl Fn(TerminalOutput) + Send + 'static,
) {
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];

        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let data = String::from_utf8_lossy(&buffer[..count]).to_string();
                    on_output(TerminalOutput {
                        session_id: session_id.clone(),
                        data,
                    });
                }
                Err(_) => break,
            }
        }
    });
}

fn spawn_terminal_exit_watcher(
    sessions: Arc<Mutex<HashMap<String, TerminalSession>>>,
    session_id: String,
    mut child: Box<dyn portable_pty::Child + Send>,
    on_exit: impl Fn(TerminalExit) + Send + 'static,
) {
    thread::spawn(move || {
        let code = child.wait().ok().map(|status| status.exit_code() as i32);

        if let Ok(mut sessions) = sessions.lock() {
            sessions.remove(&session_id);
        }

        on_exit(TerminalExit { session_id, code });
    });
}

fn default_shell() -> String {
    env::var("SHELL")
        .ok()
        .filter(|shell| !shell.trim().is_empty())
        .unwrap_or_else(|| "/bin/zsh".to_string())
}

fn new_session_id() -> String {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("terminal-{timestamp}")
}

fn terminal_size(cols: u16, rows: u16) -> PtySize {
    PtySize {
        cols: cols.max(2),
        rows: rows.max(1),
        pixel_width: 0,
        pixel_height: 0,
    }
}

fn validated_cwd(cwd: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(cwd);

    if !path.is_dir() {
        return Err(format!("Terminal working directory does not exist: {cwd}"));
    }

    Ok(path)
}
