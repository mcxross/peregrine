use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

pub const SERVER_NAME: &str = "peregrine-sui-move-analyzer";
pub const SERVER_BINARY_NAME: &str = "peregrine-sui-move-analyzer-mcp-server";
pub const SERVER_PATH_ENV: &str = "PEREGRINE_SUI_MOVE_ANALYZER_MCP_SERVER_PATH";
pub const ADAPTER_SOURCE_ENV: &str = "PEREGRINE_SUI_MOVE_ANALYZER_SOURCE";
pub const BINARY_PATH_ENV: &str = "PEREGRINE_SUI_MOVE_ANALYZER_BINARY_PATH";
pub const MAX_SOURCE_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: usize = 256 * 1024;
pub const MAX_COMPLETION_ITEMS: usize = 200;
pub const MAX_LOCATIONS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveAnalyzerToolsConfig {
    pub mode: MoveAnalyzerToolsMode,
    pub adapter: MoveAnalyzerAdapterSettings,
}

impl Default for MoveAnalyzerToolsConfig {
    fn default() -> Self {
        Self {
            mode: MoveAnalyzerToolsMode::Auto,
            adapter: MoveAnalyzerAdapterSettings::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MoveAnalyzerToolsMode {
    #[default]
    Auto,
    Always,
    Disabled,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MoveAnalyzerAdapterSettings {
    pub source: MoveAnalyzerAdapterSource,
    pub binary_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum MoveAnalyzerAdapterSource {
    #[default]
    Bundled,
    System,
}

impl MoveAnalyzerAdapterSource {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bundled => "bundled",
            Self::System => "system",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentArgs {
    #[serde(default)]
    pub project_root: Option<String>,
    pub path: String,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionArgs {
    #[serde(flatten)]
    pub document: DocumentArgs,
    pub position: Position,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionArgs {
    #[serde(flatten)]
    pub document: DocumentArgs,
    pub position: Position,
    #[serde(default)]
    pub context: Option<Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameArgs {
    #[serde(flatten)]
    pub document: DocumentArgs,
    pub position: Position,
    pub new_name: String,
}

pub mod tool_name {
    pub const STATUS: &str = "status";
    pub const DIAGNOSTICS: &str = "diagnostics";
    pub const COMPLETION: &str = "completion";
    pub const HOVER: &str = "hover";
    pub const DEFINITION: &str = "definition";
    pub const REFERENCES: &str = "references";
    pub const RENAME: &str = "rename";

    pub const ALL: &[&str] = &[
        STATUS,
        DIAGNOSTICS,
        COMPLETION,
        HOVER,
        DEFINITION,
        REFERENCES,
        RENAME,
    ];
}

pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
}

pub fn tool_definitions() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: tool_name::STATUS,
            description: "Report Move Analyzer availability and the active adapter.",
            input_schema: object_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: tool_name::DIAGNOSTICS,
            description: "Analyze a Sui Move document and return bounded compiler diagnostics.",
            input_schema: document_schema(json!({}), &[]),
        },
        ToolDefinition {
            name: tool_name::COMPLETION,
            description: "Return Sui Move completion candidates at a zero-based source position.",
            input_schema: document_schema(
                json!({
                    "position": position_schema(),
                    "context": {"type": ["object", "null"]}
                }),
                &["position"],
            ),
        },
        ToolDefinition {
            name: tool_name::HOVER,
            description: "Return Sui Move hover information at a zero-based source position.",
            input_schema: document_schema(json!({"position": position_schema()}), &["position"]),
        },
        ToolDefinition {
            name: tool_name::DEFINITION,
            description: "Resolve definitions for the Sui Move symbol at a source position.",
            input_schema: document_schema(json!({"position": position_schema()}), &["position"]),
        },
        ToolDefinition {
            name: tool_name::REFERENCES,
            description: "Resolve references for the Sui Move symbol at a source position.",
            input_schema: document_schema(json!({"position": position_schema()}), &["position"]),
        },
        ToolDefinition {
            name: tool_name::RENAME,
            description: "Preview Sui Move rename edits without modifying files.",
            input_schema: document_schema(
                json!({
                    "position": position_schema(),
                    "newName": {"type": "string", "minLength": 1, "maxLength": 256}
                }),
                &["position", "newName"],
            ),
        },
    ]
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

fn document_schema(extra: Value, required_extra: &[&str]) -> Value {
    let mut properties = json!({
        "projectRoot": {
            "type": ["string", "null"],
            "description": "Optional path relative to the MCP workspace."
        },
        "path": {
            "type": "string",
            "minLength": 1,
            "description": "Move source path relative to projectRoot."
        },
        "source": {
            "type": ["string", "null"],
            "description": "Optional unsaved source snapshot. Omit to read the file from disk."
        }
    });
    if let (Some(properties), Some(extra)) = (properties.as_object_mut(), extra.as_object()) {
        properties.extend(extra.clone());
    }
    let mut required = vec!["path"];
    required.extend(required_extra.iter().copied());
    object_schema(properties, &required)
}

fn position_schema() -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "line": {"type": "integer", "minimum": 0},
            "character": {"type": "integer", "minimum": 0}
        },
        "required": ["line", "character"]
    })
}

fn object_schema(properties: Value, required: &[&str]) -> Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
    })
}

fn server_binary_file_name() -> &'static str {
    if cfg!(windows) {
        "peregrine-sui-move-analyzer-mcp-server.exe"
    } else {
        SERVER_BINARY_NAME
    }
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
