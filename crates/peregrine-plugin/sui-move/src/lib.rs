mod index;
mod install;
mod server;

use anyhow::Context;
use include_dir::{Dir, include_dir};
use rmcp::{ServiceExt, transport::stdio};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub use index::{
    CorpusIndex, KnowledgeChunk, KnowledgeIndex, KnowledgeIndexError, MAX_CHUNK_TOKENS,
    MAX_RESPONSE_TOKENS, MAX_SEARCH_RESULTS, SearchResult, TrustTier,
};
pub use install::{
    InstalledKnowledgePlugin, KnowledgeInstallError, PLUGIN_CONFIG_KEY, bundled_cache_root_dir,
    install_bundled_plugin,
};
pub use server::SuiMoveKnowledgeServer;

pub const SERVER_NAME: &str = "peregrine-sui-move-knowledge";
pub const SERVER_BINARY_NAME: &str = "peregrine-sui-move-knowledge";
pub const SERVER_PATH_ENV: &str = "PEREGRINE_SUI_MOVE_KNOWLEDGE_SERVER_PATH";
pub const KNOWLEDGE_ROOT_ENV: &str = "PEREGRINE_SUI_MOVE_KNOWLEDGE_ROOT";

pub static BUNDLED_CORPUS: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/knowledge/sui-move");

pub mod tool_name {
    pub const KNOWLEDGE_SEARCH: &str = "knowledge_search";
    pub const KNOWLEDGE_READ: &str = "knowledge_read";
    pub const SECURITY_RULES: &str = "security_rules";

    pub const ALL: &[&str] = &[KNOWLEDGE_SEARCH, KNOWLEDGE_READ, SECURITY_RULES];
}

#[derive(Clone, Debug)]
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: serde_json::Value,
}

pub fn tool_definitions() -> Vec<ToolDefinition> {
    use serde_json::json;
    use tool_name::*;

    vec![
        ToolDefinition {
            name: KNOWLEDGE_SEARCH,
            description: "Search the bundled Sui Move security knowledge corpus. Results are advisory and do not count as verification evidence.",
            input_schema: object_schema(
                json!({
                    "query": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Specific Sui Move or Sui security topic to look up."
                    },
                    "limit": {
                        "type": ["integer", "null"],
                        "minimum": 1,
                        "maximum": MAX_SEARCH_RESULTS
                    }
                }),
                &["query"],
            ),
        },
        ToolDefinition {
            name: KNOWLEDGE_READ,
            description: "Read one indexed Sui Move knowledge chunk by chunk ID returned from knowledge_search.",
            input_schema: object_schema(
                json!({
                    "chunkId": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Indexed chunk ID returned by knowledge_search."
                    }
                }),
                &["chunkId"],
            ),
        },
        ToolDefinition {
            name: SECURITY_RULES,
            description: "Return bounded Sui Move security rule prompts for hypothesis generation. These rules are advisory only.",
            input_schema: object_schema(
                json!({
                    "category": {
                        "type": ["string", "null"],
                        "description": "Optional rule category such as access-control, arithmetic, object-model, upgradeability, or testing."
                    },
                    "limit": {
                        "type": ["integer", "null"],
                        "minimum": 1,
                        "maximum": 50
                    }
                }),
                &[],
            ),
        },
    ]
}

pub fn run_stdio() -> anyhow::Result<()> {
    tokio::runtime::Builder::new_multi_thread()
        .thread_stack_size(16 * 1024 * 1024)
        .enable_all()
        .build()
        .context("create Sui Move knowledge MCP runtime")?
        .block_on(run_stdio_async())
}

async fn run_stdio_async() -> anyhow::Result<()> {
    let service = SuiMoveKnowledgeServer::from_environment()?
        .serve(stdio())
        .await
        .context("start Sui Move knowledge MCP server")?;
    service
        .waiting()
        .await
        .context("run Sui Move knowledge MCP server")?;
    Ok(())
}

pub fn resolve_server_executable_from(
    current_exe: Option<&Path>,
    injected_path: Option<OsString>,
    path: Option<OsString>,
) -> PathBuf {
    if let Some(injected_path) = injected_path {
        let injected_path = PathBuf::from(injected_path);
        if injected_path.is_file() {
            return injected_path;
        }
    }
    if let Some(current_exe) = current_exe {
        let resolved_current_exe = current_exe
            .canonicalize()
            .unwrap_or_else(|_| current_exe.to_path_buf());
        let sibling = resolved_current_exe.with_file_name(server_binary_file_name());
        if sibling.is_file() {
            return sibling;
        }
    }
    if let Some(path) = path {
        for directory in std::env::split_paths(&path) {
            let candidate = directory.join(server_binary_file_name());
            if candidate.is_file() {
                return candidate;
            }
        }
    }
    if let Some(current_exe) = current_exe {
        let resolved_current_exe = current_exe
            .canonicalize()
            .unwrap_or_else(|_| current_exe.to_path_buf());
        return resolved_current_exe.with_file_name(server_binary_file_name());
    }
    PathBuf::from(SERVER_BINARY_NAME)
}

fn server_binary_file_name() -> &'static str {
    if cfg!(windows) {
        "peregrine-sui-move-knowledge.exe"
    } else {
        SERVER_BINARY_NAME
    }
}

fn object_schema(properties: serde_json::Value, required: &[&str]) -> serde_json::Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_server_executable_from, server_binary_file_name, tool_definitions, tool_name,
    };
    use std::fs;

    #[test]
    fn exposes_exact_tool_inventory() {
        assert_eq!(
            tool_definitions()
                .into_iter()
                .map(|definition| definition.name)
                .collect::<Vec<_>>(),
            tool_name::ALL
        );
    }

    #[test]
    fn missing_server_resolves_to_the_dedicated_sibling_path()
    -> Result<(), Box<dyn std::error::Error>> {
        let temp = tempfile::tempdir()?;
        let current = temp.path().join("bin/peregrine");
        let expected = current.with_file_name(server_binary_file_name());
        let Some(parent) = current.parent() else {
            panic!("test executable path has no parent");
        };
        fs::create_dir_all(parent)?;
        fs::write(&current, "")?;

        assert_eq!(
            resolve_server_executable_from(Some(&current), None, None),
            expected
        );
        Ok(())
    }
}
