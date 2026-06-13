use crate::core::{Diagnostic, DiagnosticSeverity, SourceSpan, stable_id};

pub fn diagnostic(
    package_id: impl AsRef<str>,
    source: impl AsRef<str>,
    message: impl AsRef<str>,
    severity: DiagnosticSeverity,
    source_span: SourceSpan,
    metadata_json: Option<serde_json::Value>,
) -> Diagnostic {
    let package_id = package_id.as_ref();
    let source = source.as_ref();
    let message = message.as_ref();
    Diagnostic {
        id: stable_id("diagnostic", [package_id, source, message]),
        package_id: package_id.to_string(),
        severity,
        source: source.to_string(),
        message: message.to_string(),
        source_span,
        metadata_json,
    }
}

pub fn malformed_summary(
    package_id: &str,
    artifact_id: &str,
    summary_path: &str,
    error: &dyn std::fmt::Display,
) -> Diagnostic {
    diagnostic(
        package_id,
        "sui.summary_loader",
        format!("Malformed package summary: {error}"),
        DiagnosticSeverity::Warning,
        SourceSpan::summary_artifact(artifact_id.to_string()),
        Some(serde_json::json!({ "summary_path": summary_path })),
    )
}
