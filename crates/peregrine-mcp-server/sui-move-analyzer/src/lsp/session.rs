use super::framing;
use peregrine_helper_protocol::{MOVE_ANALYZER_HELPER_ARG, resolve_helper_executable};
use peregrine_sui_move_analyzer::{MoveAnalyzerExecutionTarget, MoveAnalyzerServerCommand};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};
use tokio::{
    io::BufReader,
    process::{Child, ChildStdin, Command},
    sync::{Mutex, Notify, oneshot},
};
use url::Url;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const DIAGNOSTICS_TIMEOUT: Duration = Duration::from_secs(2);

type PendingResponse = oneshot::Sender<Result<Value, String>>;

pub struct LspSession {
    root: PathBuf,
    child: Mutex<Child>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, PendingResponse>>>,
    documents: Mutex<HashMap<String, DocumentState>>,
    diagnostics: Arc<Mutex<HashMap<String, Value>>>,
    diagnostics_changed: Arc<Notify>,
    next_request_id: AtomicU64,
}

struct DocumentState {
    source: String,
    version: u64,
}

impl LspSession {
    pub async fn start(
        root: PathBuf,
        command: MoveAnalyzerServerCommand,
    ) -> Result<Arc<Self>, String> {
        let mut child = spawn_process(&root, &command)?;
        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Move Analyzer process did not expose stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Move Analyzer process did not expose stdout".to_string())?;
        let stdin = Arc::new(Mutex::new(stdin));
        let pending = Arc::new(Mutex::new(HashMap::new()));
        let diagnostics = Arc::new(Mutex::new(HashMap::new()));
        let diagnostics_changed = Arc::new(Notify::new());
        let session = Arc::new(Self {
            root,
            child: Mutex::new(child),
            stdin: Arc::clone(&stdin),
            pending: Arc::clone(&pending),
            documents: Mutex::new(HashMap::new()),
            diagnostics: Arc::clone(&diagnostics),
            diagnostics_changed: Arc::clone(&diagnostics_changed),
            next_request_id: AtomicU64::new(1),
        });
        tokio::spawn(read_messages(
            BufReader::new(stdout),
            stdin,
            pending,
            diagnostics,
            diagnostics_changed,
        ));
        session.initialize().await?;
        Ok(session)
    }

    pub async fn is_alive(&self) -> bool {
        self.child
            .lock()
            .await
            .try_wait()
            .is_ok_and(|status| status.is_none())
    }

    pub async fn shutdown(&self) {
        let _ = self.child.lock().await.kill().await;
    }

    pub async fn ensure_document(&self, path: &Path, source: String) -> Result<String, String> {
        let uri = file_uri(path)?;
        let update = {
            let mut documents = self.documents.lock().await;
            match documents.get_mut(&uri) {
                Some(document) if document.source == source => None,
                Some(document) => {
                    document.version += 1;
                    document.source.clone_from(&source);
                    Some((
                        "textDocument/didChange",
                        json!({
                            "textDocument": {"uri": uri, "version": document.version},
                            "contentChanges": [{"text": source}],
                        }),
                    ))
                }
                None => {
                    documents.insert(
                        uri.clone(),
                        DocumentState {
                            source: source.clone(),
                            version: 1,
                        },
                    );
                    Some((
                        "textDocument/didOpen",
                        json!({
                            "textDocument": {
                                "uri": uri,
                                "languageId": "move",
                                "version": 1,
                                "text": source,
                            },
                        }),
                    ))
                }
            }
        };
        if let Some((method, params)) = update {
            self.diagnostics.lock().await.remove(&uri);
            self.notify(method, params).await?;
        }
        Ok(uri)
    }

    pub async fn request(&self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_request_id.fetch_add(1, Ordering::Relaxed);
        let (response_tx, response_rx) = oneshot::channel();
        self.pending.lock().await.insert(id, response_tx);
        if let Err(error) = self
            .write(&json!({
                "jsonrpc": "2.0",
                "id": id,
                "method": method,
                "params": params,
            }))
            .await
        {
            self.pending.lock().await.remove(&id);
            return Err(error);
        }
        tokio::time::timeout(REQUEST_TIMEOUT, response_rx)
            .await
            .map_err(|_| format!("Move Analyzer request timed out: {method}"))?
            .map_err(|_| format!("Move Analyzer stopped while handling: {method}"))?
    }

    pub async fn diagnostics(&self, uri: &str) -> (Value, bool) {
        let notified = self.diagnostics_changed.notified();
        if let Some(diagnostics) = self.diagnostics.lock().await.get(uri).cloned() {
            return (diagnostics, true);
        }
        let fresh = tokio::time::timeout(DIAGNOSTICS_TIMEOUT, notified)
            .await
            .is_ok();
        let diagnostics = self
            .diagnostics
            .lock()
            .await
            .get(uri)
            .cloned()
            .unwrap_or_else(|| json!([]));
        (diagnostics, fresh)
    }

    async fn initialize(&self) -> Result<(), String> {
        let root_uri = file_uri(&self.root)?;
        self.request(
            "initialize",
            json!({
                "processId": null,
                "rootUri": root_uri,
                "workspaceFolders": [{
                    "name": self.root.file_name().and_then(|name| name.to_str()).unwrap_or("workspace"),
                    "uri": root_uri,
                }],
                "capabilities": {
                    "textDocument": {
                        "completion": {
                            "completionItem": {
                                "documentationFormat": ["markdown", "plaintext"],
                                "snippetSupport": false
                            }
                        },
                        "hover": {"contentFormat": ["markdown", "plaintext"]},
                        "publishDiagnostics": {"relatedInformation": false},
                        "definition": {},
                        "references": {},
                        "rename": {"prepareSupport": false}
                    },
                    "workspace": {"configuration": false}
                }
            }),
        )
        .await?;
        self.notify("initialized", json!({})).await
    }

    async fn notify(&self, method: &str, params: Value) -> Result<(), String> {
        self.write(&json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        }))
        .await
    }

    async fn write(&self, message: &Value) -> Result<(), String> {
        framing::write_message(&mut *self.stdin.lock().await, message)
            .await
            .map_err(|error| format!("Could not send Move Analyzer message: {error}"))
    }
}

fn spawn_process(root: &Path, command: &MoveAnalyzerServerCommand) -> Result<Child, String> {
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
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .kill_on_drop(true)
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

async fn read_messages(
    mut stdout: BufReader<tokio::process::ChildStdout>,
    stdin: Arc<Mutex<ChildStdin>>,
    pending: Arc<Mutex<HashMap<u64, PendingResponse>>>,
    diagnostics: Arc<Mutex<HashMap<String, Value>>>,
    diagnostics_changed: Arc<Notify>,
) {
    while let Ok(Some(message)) = framing::read_message(&mut stdout).await {
        if message.get("method").and_then(Value::as_str) == Some("textDocument/publishDiagnostics")
            && let Some(params) = message.get("params")
            && let (Some(uri), Some(items)) = (
                params.get("uri").and_then(Value::as_str),
                params.get("diagnostics"),
            )
        {
            diagnostics
                .lock()
                .await
                .insert(uri.to_string(), items.clone());
            diagnostics_changed.notify_waiters();
            continue;
        }
        if let Some(id) = message.get("id").and_then(Value::as_u64) {
            if message.get("method").is_some() {
                let _ = framing::write_message(
                    &mut *stdin.lock().await,
                    &json!({"jsonrpc": "2.0", "id": id, "result": null}),
                )
                .await;
                continue;
            }
            if let Some(response) = pending.lock().await.remove(&id) {
                let result = match message.get("error") {
                    Some(error) => Err(format!("Move Analyzer request failed: {error}")),
                    None => Ok(message.get("result").cloned().unwrap_or(Value::Null)),
                };
                let _ = response.send(result);
            }
        }
    }
    for (_, response) in pending.lock().await.drain() {
        let _ = response.send(Err("Move Analyzer process exited".to_string()));
    }
}

fn file_uri(path: &Path) -> Result<String, String> {
    Url::from_file_path(path)
        .map(|url| url.to_string())
        .map_err(|()| format!("Could not convert {} to a file URI", path.display()))
}
