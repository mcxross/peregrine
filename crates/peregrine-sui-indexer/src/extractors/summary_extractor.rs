use std::{collections::BTreeMap, fs, path::Path};

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, Deserialize)]
pub struct SummaryModule {
    pub id: SummaryModuleId,
    #[serde(default)]
    pub immediate_dependencies: Vec<SummaryModuleId>,
    #[serde(default)]
    pub functions: BTreeMap<String, Value>,
    #[serde(default)]
    pub structs: BTreeMap<String, Value>,
    #[serde(default)]
    pub enums: BTreeMap<String, Value>,
    #[serde(default)]
    pub friends: Vec<SummaryModuleId>,
    #[serde(default)]
    pub docs: Option<String>,
    #[serde(default)]
    pub attributes: Value,
    #[serde(default)]
    pub schema_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SummaryModuleId {
    pub address: String,
    pub name: String,
}

pub fn read_summary_module(path: &Path) -> crate::core::IndexerResult<SummaryModule> {
    let source = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&source)?)
}

pub fn derived_summary_identity(summary_root: Option<&Path>, path: &Path) -> (String, String) {
    let module_name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_string();
    let package_alias = summary_root
        .and_then(|root| path.strip_prefix(root).ok())
        .and_then(|relative| relative.components().next())
        .and_then(|component| component.as_os_str().to_str())
        .unwrap_or("unknown")
        .to_string();
    (package_alias, module_name)
}

pub fn module_card_json(
    summary: &SummaryModule,
    dependency: bool,
    symbol_name: Option<&str>,
) -> Value {
    let selected_public_symbols = summary
        .functions
        .iter()
        .filter(|(name, value)| {
            symbol_name.is_none_or(|target| *name == target)
                && value
                    .get("visibility")
                    .map(|visibility| format!("{visibility:?}").contains("Public"))
                    .unwrap_or(false)
        })
        .take(if dependency { 24 } else { usize::MAX })
        .map(|(name, value)| {
            serde_json::json!({
                "name": name,
                "kind": "function",
                "visibility": value.get("visibility").cloned().unwrap_or(Value::Null),
                "entry": value.get("entry").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "package_alias": summary.id.address,
        "module_name": summary.id.name,
        "immediate_dependencies": summary.immediate_dependencies.iter().map(|dep| format!("{}::{}", dep.address, dep.name)).collect::<Vec<_>>(),
        "public_functions_count": selected_public_symbols.len(),
        "types_count": summary.structs.len() + summary.enums.len(),
        "selected_public_symbols": selected_public_symbols,
    })
}
