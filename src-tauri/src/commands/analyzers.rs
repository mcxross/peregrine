use std::path::PathBuf;

use peregrine_sui_static_analysis::{
    AnalysisConfig, AnalysisEngine, AnalysisEngineOptions, AnalysisPluginHost, AnalysisRuleCatalog,
    AnalyzerPluginRegistry, InstalledAnalyzerPlugin, RuleConfig, Severity,
};
use serde::Deserialize;
use tauri::Manager;

use crate::commands::files::resolve_package_child_path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AnalysisRuleConfigPatch {
    ruleset_id: String,
    rule_id: Option<String>,
    active: Option<bool>,
    severity: Option<Severity>,
    threshold: Option<u32>,
    entry_threshold: Option<u32>,
}

#[tauri::command]
pub(crate) async fn list_analyzer_plugins(
    app: tauri::AppHandle,
) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
    tauri::async_runtime::spawn_blocking(move || analyzer_registry(&app)?.list_plugins())
        .await
        .map_err(|error| format!("Could not join analyzer plugin list task: {error}"))?
}

#[tauri::command]
pub(crate) async fn install_analyzer_plugin(
    app: tauri::AppHandle,
    plugin_path: String,
) -> Result<InstalledAnalyzerPlugin, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let registry = analyzer_registry(&app)?;
        registry.install_plugin(PathBuf::from(plugin_path), &AnalysisPluginHost)
    })
    .await
    .map_err(|error| format!("Could not join analyzer plugin install task: {error}"))?
}

#[tauri::command]
pub(crate) async fn remove_analyzer_plugin(
    app: tauri::AppHandle,
    plugin_id: String,
) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
    tauri::async_runtime::spawn_blocking(move || analyzer_registry(&app)?.remove_plugin(&plugin_id))
        .await
        .map_err(|error| format!("Could not join analyzer plugin removal task: {error}"))?
}

#[tauri::command]
pub(crate) async fn set_analyzer_plugin_enabled(
    app: tauri::AppHandle,
    plugin_id: String,
    enabled: bool,
) -> Result<Vec<InstalledAnalyzerPlugin>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        analyzer_registry(&app)?.set_plugin_enabled(&plugin_id, enabled)
    })
    .await
    .map_err(|error| format!("Could not join analyzer plugin update task: {error}"))?
}

#[tauri::command]
pub(crate) async fn list_analyzer_rule_catalog(
    app: tauri::AppHandle,
    root_path: Option<String>,
    package_path: Option<String>,
) -> Result<AnalysisRuleCatalog, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let registry_root = analyzer_registry_root(&app)?;
        let package_root = match (root_path, package_path) {
            (Some(root_path), Some(package_path)) => {
                resolve_package_child_path(&root_path, &package_path)?
            }
            _ => std::env::current_dir()
                .map_err(|error| format!("Could not resolve current directory: {error}"))?,
        };
        let config = AnalysisConfig::load_from_package(&package_root)?;

        Ok(AnalysisEngine::new().catalog_with_options(
            &package_root,
            config,
            AnalysisEngineOptions {
                global_plugin_root: Some(registry_root),
                ..AnalysisEngineOptions::default()
            },
        ))
    })
    .await
    .map_err(|error| format!("Could not join analyzer rule catalog task: {error}"))?
}

#[tauri::command]
pub(crate) async fn save_analysis_rule_config(
    root_path: String,
    package_path: String,
    patch: AnalysisRuleConfigPatch,
) -> Result<AnalysisConfig, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let package_root = resolve_package_child_path(&root_path, &package_path)?;
        let mut config = AnalysisConfig::load_from_package(&package_root)?.with_defaults();
        let ruleset_id = patch.ruleset_id.clone();
        let ruleset = config
            .analysis
            .rulesets
            .entry(ruleset_id.clone())
            .or_default();

        if patch_targets_ruleset(&ruleset_id, patch.rule_id.as_deref()) {
            if patch.active.is_some() {
                ruleset.active = patch.active;
            }
            if patch.severity.is_some() {
                ruleset.severity = patch.severity;
            }
            if patch.threshold.is_some() {
                ruleset.threshold = patch.threshold;
            }
            if patch.entry_threshold.is_some() {
                ruleset.entry_threshold = patch.entry_threshold;
            }
        } else if let Some(rule_id) = patch.rule_id {
            let rule = ruleset
                .rules
                .entry(rule_id)
                .or_insert_with(RuleConfig::default);

            if patch.active.is_some() {
                rule.active = patch.active;
            }
            if patch.severity.is_some() {
                rule.severity = patch.severity;
            }
            if patch.threshold.is_some() {
                rule.threshold = patch.threshold;
            }
            if patch.entry_threshold.is_some() {
                rule.entry_threshold = patch.entry_threshold;
            }
        } else if patch.active.is_some() {
            ruleset.active = patch.active;
        }

        config.save_to_package(&package_root)?;
        Ok(config)
    })
    .await
    .map_err(|error| format!("Could not join analyzer rule config save task: {error}"))?
}

pub(crate) fn analyzer_registry(app: &tauri::AppHandle) -> Result<AnalyzerPluginRegistry, String> {
    analyzer_registry_root(app).map(AnalyzerPluginRegistry::at_root)
}

fn analyzer_registry_root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    app.path()
        .app_config_dir()
        .map_err(|error| format!("Could not resolve app config directory: {error}"))
}

fn patch_targets_ruleset(ruleset_id: &str, rule_id: Option<&str>) -> bool {
    rule_id == Some(ruleset_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snake_case_single_rule_patch_targets_ruleset_config() {
        assert!(patch_targets_ruleset(
            "unchecked_return",
            Some("unchecked_return")
        ));
        assert!(!patch_targets_ruleset(
            "complexity",
            Some("function_complexity")
        ));
    }
}
