use crate::lsp::LspManager;
use peregrine_sui_move_analyzer::{
    MoveAnalyzerAdapter, MoveAnalyzerAdapterEnvironment,
    MoveAnalyzerAdapterSettings as AdapterSettings, MoveAnalyzerAdapterSource as AdapterSource,
};
use peregrine_sui_move_analyzer_mcp_protocol::{
    CompletionArgs, DocumentArgs, MAX_COMPLETION_ITEMS, MAX_LOCATIONS, MAX_OUTPUT_BYTES,
    MAX_SOURCE_BYTES, MoveAnalyzerAdapterSettings, MoveAnalyzerAdapterSource, PositionArgs,
    RenameArgs, tool_definitions, tool_name,
};
use rmcp::{
    ErrorData, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResult, Content, Implementation, JsonObject,
        ListToolsResult, PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool,
        ToolAnnotations,
    },
    service::{RequestContext, RoleServer},
};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value, json};
use std::{
    borrow::Cow,
    path::{Component, Path, PathBuf},
    sync::Arc,
};
use url::Url;

#[derive(Clone)]
pub struct SuiMoveAnalyzerMcpServer {
    workspace_root: PathBuf,
    adapter: Arc<MoveAnalyzerAdapter>,
    lsp: Arc<LspManager>,
}

impl SuiMoveAnalyzerMcpServer {
    pub fn new(
        workspace_root: PathBuf,
        settings: MoveAnalyzerAdapterSettings,
    ) -> anyhow::Result<Self> {
        let workspace_root = workspace_root.canonicalize()?;
        let adapter = MoveAnalyzerAdapter::new(
            AdapterSettings {
                source: match settings.source {
                    MoveAnalyzerAdapterSource::Bundled => AdapterSource::BundledLibrary,
                    MoveAnalyzerAdapterSource::System => AdapterSource::System,
                },
                binary_path: settings.binary_path,
            },
            MoveAnalyzerAdapterEnvironment::new(),
        );
        Ok(Self {
            workspace_root,
            lsp: Arc::new(LspManager::new(MoveAnalyzerAdapter::new(
                adapter.settings().clone(),
                MoveAnalyzerAdapterEnvironment::new(),
            ))),
            adapter: Arc::new(adapter),
        })
    }

    async fn dispatch(&self, request: CallToolRequestParams) -> Result<CallToolResult, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        let value = match request.name.as_ref() {
            tool_name::STATUS => self.status_value(),
            tool_name::DIAGNOSTICS => {
                let args = parse_args::<DocumentArgs>(&arguments)?;
                let document = self.resolve_document(args).map_err(tool_error)?;
                let session = self
                    .lsp
                    .session(&document.project_root)
                    .await
                    .map_err(tool_error)?;
                let uri = session
                    .ensure_document(&document.path, document.source)
                    .await
                    .map_err(tool_error)?;
                let (diagnostics, fresh) = session.diagnostics(&uri).await;
                json!({
                    "path": document.relative_path,
                    "diagnostics": normalize_diagnostics(diagnostics),
                    "fresh": fresh,
                    "warnings": if fresh {
                        Vec::<String>::new()
                    } else {
                        vec!["Move Analyzer did not publish diagnostics before the 2 second wait elapsed.".to_string()]
                    },
                })
            }
            tool_name::COMPLETION => {
                let args = parse_args::<CompletionArgs>(&arguments)?;
                let position = args.position;
                let context = args.context;
                let (session, document, uri) = self.prepare(args.document).await?;
                let mut result = session
                    .request(
                        "textDocument/completion",
                        json!({
                            "textDocument": {"uri": uri},
                            "position": position,
                            "context": context,
                        }),
                    )
                    .await
                    .map_err(tool_error)?;
                bound_completion(&mut result);
                json!({"path": document.relative_path, "completion": result})
            }
            tool_name::HOVER => {
                let args = parse_args::<PositionArgs>(&arguments)?;
                let position = args.position;
                let (session, document, uri) = self.prepare(args.document).await?;
                let result = session
                    .request(
                        "textDocument/hover",
                        json!({"textDocument": {"uri": uri}, "position": position}),
                    )
                    .await
                    .map_err(tool_error)?;
                json!({"path": document.relative_path, "hover": result})
            }
            tool_name::DEFINITION | tool_name::REFERENCES => {
                let args = parse_args::<PositionArgs>(&arguments)?;
                let position = args.position;
                let (session, document, uri) = self.prepare(args.document).await?;
                let method = if request.name.as_ref() == tool_name::DEFINITION {
                    "textDocument/definition"
                } else {
                    "textDocument/references"
                };
                let params = if method == "textDocument/references" {
                    json!({
                        "textDocument": {"uri": uri},
                        "position": position,
                        "context": {"includeDeclaration": true},
                    })
                } else {
                    json!({"textDocument": {"uri": uri}, "position": position})
                };
                let result = session.request(method, params).await.map_err(tool_error)?;
                json!({
                    "path": document.relative_path,
                    "locations": normalize_locations(&document.project_root, result),
                })
            }
            tool_name::RENAME => {
                let args = parse_args::<RenameArgs>(&arguments)?;
                if args.new_name.trim().is_empty() {
                    return Err(tool_error("newName must not be empty".to_string()));
                }
                let position = args.position;
                let new_name = args.new_name;
                let (session, document, uri) = self.prepare(args.document).await?;
                let result = session
                    .request(
                        "textDocument/rename",
                        json!({
                            "textDocument": {"uri": uri},
                            "position": position,
                            "newName": new_name,
                        }),
                    )
                    .await
                    .map_err(tool_error)?;
                json!({
                    "path": document.relative_path,
                    "edit": normalize_workspace_edit(&document.project_root, result),
                })
            }
            name => {
                return Err(ErrorData::invalid_params(
                    format!("unknown tool `{name}`"),
                    None,
                ));
            }
        };
        bounded_structured_result(value)
    }

    async fn prepare(
        &self,
        args: DocumentArgs,
    ) -> Result<(Arc<crate::lsp::LspSession>, ResolvedDocument, String), ErrorData> {
        let document = self.resolve_document(args).map_err(tool_error)?;
        let session = self
            .lsp
            .session(&document.project_root)
            .await
            .map_err(tool_error)?;
        let uri = session
            .ensure_document(&document.path, document.source.clone())
            .await
            .map_err(tool_error)?;
        Ok((session, document, uri))
    }

    fn resolve_document(&self, args: DocumentArgs) -> Result<ResolvedDocument, String> {
        let project_root = self.resolve_project_root(args.project_root.as_deref())?;
        let relative = Path::new(args.path.trim());
        if relative.as_os_str().is_empty()
            || relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err("path must be a non-empty relative path inside projectRoot".to_string());
        }
        let path = project_root.join(relative);
        let canonical = path.canonicalize().map_err(|error| {
            format!("failed to resolve Move source {}: {error}", path.display())
        })?;
        if !canonical.starts_with(&project_root) || !canonical.is_file() {
            return Err("path must resolve to a file inside projectRoot".to_string());
        }
        let source = match args.source {
            Some(source) => source,
            None => std::fs::read_to_string(&canonical).map_err(|error| {
                format!(
                    "failed to read Move source {}: {error}",
                    canonical.display()
                )
            })?,
        };
        if source.len() > MAX_SOURCE_BYTES {
            return Err(format!("source exceeds the {MAX_SOURCE_BYTES} byte limit"));
        }
        Ok(ResolvedDocument {
            project_root,
            path: canonical,
            relative_path: relative.to_string_lossy().replace('\\', "/"),
            source,
        })
    }

    fn resolve_project_root(&self, project_root: Option<&str>) -> Result<PathBuf, String> {
        let project_root = project_root
            .filter(|value| !value.trim().is_empty())
            .map_or_else(
                || self.workspace_root.clone(),
                |value| self.workspace_root.join(value),
            )
            .canonicalize()
            .map_err(|error| format!("failed to resolve projectRoot: {error}"))?;
        if !project_root.starts_with(&self.workspace_root) || !project_root.is_dir() {
            return Err(format!(
                "projectRoot must be a directory inside the MCP workspace {}",
                self.workspace_root.display()
            ));
        }
        Ok(project_root)
    }

    fn status_value(&self) -> Value {
        let status = self.adapter.status();
        json!({
            "installed": status.installed,
            "version": status.version,
            "installHint": status.install_hint,
            "activeSource": status.active_source.map(source_name),
            "preferredSource": source_name(status.preferred_source),
            "resolvedPath": status.resolved_path,
            "bundled": {
                "source": "bundled",
                "available": status.bundled.available,
                "version": status.bundled.version,
                "path": status.bundled.path,
                "error": status.bundled.error,
            },
            "system": {
                "source": "system",
                "available": status.system.available,
                "version": status.system.version,
                "path": status.system.path,
                "error": status.system.error,
            },
        })
    }
}

impl ServerHandler for SuiMoveAnalyzerMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                .with_title("Peregrine Sui Move Analyzer"),
        )
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let tools = tool_definitions()
            .into_iter()
            .map(|definition| {
                let input_schema = serde_json::from_value::<JsonObject>(definition.input_schema)
                    .map_err(serialization_error)?;
                let mut tool = Tool::new(
                    Cow::Borrowed(definition.name),
                    Cow::Borrowed(definition.description),
                    Arc::new(input_schema),
                );
                tool.annotations = Some(
                    ToolAnnotations::new()
                        .read_only(true)
                        .destructive(false)
                        .open_world(false),
                );
                Ok(tool)
            })
            .collect::<Result<Vec<_>, ErrorData>>()?;
        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, ErrorData> {
        match self.dispatch(request).await {
            Ok(result) => Ok(result),
            Err(error) => Ok(CallToolResult::structured_error(json!({
                "status": "error",
                "message": error.message,
            }))),
        }
    }
}

struct ResolvedDocument {
    project_root: PathBuf,
    path: PathBuf,
    relative_path: String,
    source: String,
}

fn parse_args<T>(arguments: &JsonObject) -> Result<T, ErrorData>
where
    T: DeserializeOwned,
{
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
}

fn source_name(source: AdapterSource) -> &'static str {
    match source {
        AdapterSource::BundledLibrary => "bundled",
        AdapterSource::System => "system",
    }
}

fn normalize_diagnostics(value: Value) -> Vec<Value> {
    value
        .as_array()
        .into_iter()
        .flatten()
        .take(MAX_LOCATIONS)
        .filter_map(|diagnostic| {
            let mut diagnostic = diagnostic.as_object()?.clone();
            let severity = match diagnostic.get("severity").and_then(Value::as_u64) {
                Some(2) => "warning",
                Some(3) => "info",
                Some(4) => "hint",
                _ => "error",
            };
            diagnostic.insert("severity".to_string(), json!(severity));
            diagnostic
                .entry("source".to_string())
                .or_insert_with(|| json!("move-analyzer"));
            Some(Value::Object(diagnostic))
        })
        .collect()
}

fn bound_completion(result: &mut Value) {
    if let Some(items) = result.as_array_mut() {
        items.truncate(MAX_COMPLETION_ITEMS);
    } else if let Some(items) = result
        .as_object_mut()
        .and_then(|result| result.get_mut("items"))
        .and_then(Value::as_array_mut)
    {
        items.truncate(MAX_COMPLETION_ITEMS);
    }
}

fn normalize_locations(root: &Path, value: Value) -> Vec<Value> {
    let values = value.as_array().cloned().unwrap_or_else(|| vec![value]);
    values
        .into_iter()
        .take(MAX_LOCATIONS)
        .filter_map(|location| {
            let object = location.as_object()?;
            let (uri, range) = match (
                object.get("uri").and_then(Value::as_str),
                object.get("range"),
            ) {
                (Some(uri), Some(range)) => (uri, range),
                _ => (
                    object.get("targetUri").and_then(Value::as_str)?,
                    object
                        .get("targetSelectionRange")
                        .or_else(|| object.get("targetRange"))?,
                ),
            };
            let path = relative_file_uri(root, uri)?;
            Some(json!({"path": path, "uri": uri, "range": range}))
        })
        .collect()
}

fn normalize_workspace_edit(root: &Path, value: Value) -> Value {
    let Some(object) = value.as_object() else {
        return Value::Null;
    };
    let mut edits_by_path = Map::new();
    if let Some(changes) = object.get("changes").and_then(Value::as_object) {
        for (uri, edits) in changes {
            add_edits(root, &mut edits_by_path, uri, edits);
        }
    }
    if let Some(changes) = object.get("documentChanges").and_then(Value::as_array) {
        for change in changes {
            let Some(change) = change.as_object() else {
                continue;
            };
            let Some(uri) = change
                .get("textDocument")
                .and_then(Value::as_object)
                .and_then(|document| document.get("uri"))
                .and_then(Value::as_str)
            else {
                continue;
            };
            if let Some(edits) = change.get("edits") {
                add_edits(root, &mut edits_by_path, uri, edits);
            }
        }
    }
    if edits_by_path.is_empty() {
        Value::Null
    } else {
        json!({"editsByPath": edits_by_path})
    }
}

fn add_edits(root: &Path, edits_by_path: &mut Map<String, Value>, uri: &str, edits: &Value) {
    let Some(path) = relative_file_uri(root, uri) else {
        return;
    };
    let Some(edits) = edits.as_array() else {
        return;
    };
    edits_by_path.insert(
        path,
        Value::Array(edits.iter().take(MAX_LOCATIONS).cloned().collect()),
    );
}

fn relative_file_uri(root: &Path, uri: &str) -> Option<String> {
    let path = Url::parse(uri).ok()?.to_file_path().ok()?;
    path.strip_prefix(root)
        .ok()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
}

fn bounded_structured_result(value: Value) -> Result<CallToolResult, ErrorData> {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|error| error.to_string());
    if text.len() > MAX_OUTPUT_BYTES {
        return Err(ErrorData::invalid_params(
            format!("tool response exceeds the {MAX_OUTPUT_BYTES} byte limit; narrow the request"),
            None,
        ));
    }
    let mut result = CallToolResult::structured(value);
    result.content = vec![Content::text(text)];
    Ok(result)
}

fn serialization_error(error: serde_json::Error) -> ErrorData {
    ErrorData::internal_error(error.to_string(), None)
}

fn tool_error(message: String) -> ErrorData {
    ErrorData::invalid_params(message, None)
}
