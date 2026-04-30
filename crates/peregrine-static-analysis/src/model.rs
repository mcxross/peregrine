use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::config::AnalysisConfig;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisContext {
    pub package_path: PathBuf,
    pub source_files: Vec<SourceFile>,
    pub modules: Vec<ParsedModule>,
    pub config: AnalysisConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    pub path: String,
    pub contents: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedModule {
    pub name: String,
    pub address: Option<String>,
    pub file: String,
    pub functions: Vec<ParsedFunction>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedFunction {
    pub module_name: String,
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub signature: String,
    pub body: String,
    pub file: String,
    pub span: Option<Span>,
    pub type_parameter_count: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metric {
    pub name: String,
    pub value: u32,
    pub threshold: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Finding {
    pub rule_id: String,
    pub ruleset_id: String,
    pub severity: Severity,
    pub message: String,
    pub file: String,
    pub span: Option<Span>,
    pub metric: Option<Metric>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuleMetric {
    pub ruleset_id: String,
    pub rule_id: String,
    pub target: String,
    pub file: Option<String>,
    pub span: Option<Span>,
    pub metric: Metric,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisDiagnostic {
    pub level: String,
    pub source: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisReport {
    pub findings: Vec<Finding>,
    pub metrics: Vec<RuleMetric>,
    pub loaded_rulesets: Vec<String>,
    pub loaded_plugins: Vec<String>,
    pub diagnostics: Vec<AnalysisDiagnostic>,
}
