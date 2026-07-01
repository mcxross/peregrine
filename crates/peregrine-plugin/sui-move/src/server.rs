#![allow(clippy::expect_used)]
#![allow(clippy::unwrap_used)]
use crate::{
    CorpusIndex, KNOWLEDGE_ROOT_ENV, KnowledgeIndex, MAX_RESPONSE_TOKENS, MAX_SEARCH_RESULTS,
    tool_definitions, tool_name,
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
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use std::{borrow::Cow, path::Path, sync::Arc};

const DEFAULT_SECURITY_RULE_LIMIT: usize = 50;
const ADVISORY_NOTICE: &str =
    "Knowledge citations guide hypotheses only; they do not satisfy verification requirements.";

#[derive(Clone)]
pub struct SuiMoveKnowledgeServer {
    index: Arc<KnowledgeIndex>,
    rules: Arc<SecurityRuleCatalog>,
}

impl SuiMoveKnowledgeServer {
    pub fn bundled() -> anyhow::Result<Self> {
        Self::new(KnowledgeIndex::bundled()?)
    }

    pub fn from_environment() -> anyhow::Result<Self> {
        let Some(root) = std::env::var_os(KNOWLEDGE_ROOT_ENV) else {
            return Self::bundled();
        };
        Self::from_installed_root(Path::new(&root))
    }

    pub fn from_installed_root(root: &Path) -> anyhow::Result<Self> {
        let bytes = std::fs::read(root.join("index.json"))?;
        let corpus = serde_json::from_slice::<CorpusIndex>(&bytes)?;
        Self::new(KnowledgeIndex::from_corpus(corpus))
    }

    pub fn new(index: KnowledgeIndex) -> anyhow::Result<Self> {
        Ok(Self {
            index: Arc::new(index),
            rules: Arc::new(security_rule_catalog()?),
        })
    }

    async fn dispatch(&self, request: CallToolRequestParams) -> Result<CallToolResult, ErrorData> {
        let arguments = request.arguments.unwrap_or_default();
        let value =
            match request.name.as_ref() {
                tool_name::KNOWLEDGE_SEARCH => {
                    let args = parse_args::<KnowledgeSearchArgs>(&arguments)?;
                    let query = args.query.trim();
                    if query.is_empty() {
                        return Err(tool_error("query must not be empty".to_string()));
                    }
                    let limit = args.limit.unwrap_or(MAX_SEARCH_RESULTS);
                    json!({
                        "status": "ok",
                        "advisoryOnly": true,
                        "notice": ADVISORY_NOTICE,
                        "corpus": self.corpus_summary(),
                        "results": self.index.search(query, limit),
                    })
                }
                tool_name::KNOWLEDGE_READ => {
                    let args = parse_args::<KnowledgeReadArgs>(&arguments)?;
                    let chunk = self.index.corpus.chunk(args.chunk_id.trim()).ok_or_else(|| {
                    tool_error(format!(
                        "chunkId `{}` is not present in the indexed Sui Move knowledge corpus",
                        args.chunk_id
                    ))
                })?;
                    json!({
                        "status": "ok",
                        "advisoryOnly": true,
                        "notice": ADVISORY_NOTICE,
                        "corpus": self.corpus_summary(),
                        "chunk": chunk,
                    })
                }
                tool_name::SECURITY_RULES => {
                    let args = parse_args::<SecurityRulesArgs>(&arguments)?;
                    let category = args
                        .category
                        .as_deref()
                        .map(str::trim)
                        .filter(|category| !category.is_empty());
                    let limit = args.limit.unwrap_or(DEFAULT_SECURITY_RULE_LIMIT);
                    let rules = self
                        .rules
                        .rules
                        .iter()
                        .filter(|rule| category.is_none_or(|category| rule.category == category))
                        .take(limit.clamp(1, DEFAULT_SECURITY_RULE_LIMIT))
                        .cloned()
                        .collect::<Vec<_>>();
                    json!({
                        "status": "ok",
                        "advisoryOnly": true,
                        "notice": ADVISORY_NOTICE,
                        "sourceCommit": self.rules.source_commit,
                        "category": category,
                        "rules": rules,
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

    fn corpus_summary(&self) -> Value {
        json!({
            "name": self.index.corpus.corpus_name,
            "version": self.index.corpus.corpus_version,
            "hash": self.index.corpus.corpus_hash,
            "chunkCount": self.index.corpus.chunks.len(),
        })
    }
}

impl ServerHandler for SuiMoveKnowledgeServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new(env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
                .with_title("Peregrine Sui Move Knowledge"),
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeSearchArgs {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct KnowledgeReadArgs {
    chunk_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SecurityRulesArgs {
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SecurityRuleCatalog {
    source_commit: String,
    rules: Vec<SecurityRule>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct SecurityRule {
    id: String,
    category: String,
    title: String,
    prompt: String,
}

fn parse_args<T>(arguments: &JsonObject) -> Result<T, ErrorData>
where
    T: DeserializeOwned,
{
    serde_json::from_value(Value::Object(arguments.clone()))
        .map_err(|error| ErrorData::invalid_params(error.to_string(), None))
}

fn security_rule_catalog() -> anyhow::Result<SecurityRuleCatalog> {
    let Some(file) = crate::BUNDLED_CORPUS.get_file("move-security-rules.json") else {
        anyhow::bail!("bundled Sui Move security rules are missing");
    };
    Ok(serde_json::from_slice(file.contents())?)
}

fn bounded_structured_result(value: Value) -> Result<CallToolResult, ErrorData> {
    let text = serde_json::to_string_pretty(&value).unwrap_or_else(|error| error.to_string());
    if !crate::index::response_within_cap(&text) {
        return Err(ErrorData::invalid_params(
            format!(
                "tool response exceeds the {MAX_RESPONSE_TOKENS} token limit; narrow the request"
            ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolRequestParams;
    use serde_json::{Map, json};

    #[tokio::test]
    async fn read_requires_indexed_chunk_id() {
        let server = SuiMoveKnowledgeServer::bundled().unwrap();

        let result = server
            .dispatch(
                CallToolRequestParams::new(tool_name::KNOWLEDGE_READ).with_arguments(
                    Map::from_iter([("chunkId".to_string(), json!("sui-move:missing:0"))]),
                ),
            )
            .await;

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn search_is_bounded() {
        let server = SuiMoveKnowledgeServer::bundled().unwrap();

        let result = server
            .dispatch(
                CallToolRequestParams::new(tool_name::KNOWLEDGE_SEARCH).with_arguments(
                    Map::from_iter([
                        ("query".to_string(), json!("shared object access control")),
                        ("limit".to_string(), json!(100)),
                    ]),
                ),
            )
            .await
            .unwrap();

        let results = result
            .structured_content
            .unwrap()
            .get("results")
            .and_then(Value::as_array)
            .cloned()
            .unwrap();
        assert!(results.len() <= MAX_SEARCH_RESULTS);
    }
}
