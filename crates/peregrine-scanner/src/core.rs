use std::path::PathBuf;

use peregrine_types::sui::move_model::MovePackageModel;
use serde::Serialize;

use crate::sui::objects::ObjectScanReport;

pub trait PackageScanner {
    fn id(&self) -> &'static str;
    fn scan(&self, input: &ScanInput<'_>) -> ScannerOutput;
}

#[derive(Clone)]
pub struct ScanInput<'a> {
    pub package_model: &'a MovePackageModel,
    pub package_root: Option<PathBuf>,
    pub build_root: Option<PathBuf>,
    pub source_mode: SourceMode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceMode {
    BestAvailable,
    CompilerOnly,
    SourceOnly,
}

impl Default for SourceMode {
    fn default() -> Self {
        Self::BestAvailable
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanReport {
    pub package_id: String,
    pub scanners: Vec<ScannerOutput>,
    pub diagnostics: Vec<ScannerDiagnostic>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind", content = "report")]
pub enum ScannerOutput {
    Objects(ObjectScanReport),
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScannerDiagnostic {
    pub scanner_id: String,
    pub severity: ScannerDiagnosticSeverity,
    pub message: String,
    pub source: EvidenceSource,
}

impl ScannerDiagnostic {
    pub fn info(
        scanner_id: impl Into<String>,
        source: EvidenceSource,
        message: impl Into<String>,
    ) -> Self {
        Self {
            scanner_id: scanner_id.into(),
            severity: ScannerDiagnosticSeverity::Info,
            message: message.into(),
            source,
        }
    }

    pub fn warning(
        scanner_id: impl Into<String>,
        source: EvidenceSource,
        message: impl Into<String>,
    ) -> Self {
        Self {
            scanner_id: scanner_id.into(),
            severity: ScannerDiagnosticSeverity::Warning,
            message: message.into(),
            source,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScannerDiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ScannerConfidence {
    High,
    Medium,
    Low,
}

impl ScannerConfidence {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Medium => "medium",
            Self::Low => "low",
        }
    }

    pub fn rank(self) -> u8 {
        match self {
            Self::High => 3,
            Self::Medium => 2,
            Self::Low => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EvidenceSource {
    Bytecode,
    Compiler,
    SourceFallback,
    Scanner,
}
