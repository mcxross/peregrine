pub mod core;
pub mod sui;

pub use core::{
    EvidenceSource, ScanInput, ScanReport, ScannerConfidence, ScannerDiagnostic,
    ScannerDiagnosticSeverity, ScannerOutput, SourceMode,
};
