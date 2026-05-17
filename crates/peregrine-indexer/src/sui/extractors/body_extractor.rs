use crate::core::{logical_id, Operation, OperationKind, PackageId, SourceSpan};

pub fn source_body_operations(
    package_id: &PackageId,
    function_id: &str,
    body: &str,
    source_span: SourceSpan,
) -> Vec<Operation> {
    body.lines()
        .enumerate()
        .filter_map(|(line_index, line)| {
            let trimmed = line.trim();
            let kind = if trimmed.contains("assert!") {
                OperationKind::Assert
            } else if trimmed.contains("abort") {
                OperationKind::Abort
            } else if trimmed.contains("::") && trimmed.contains('(') {
                OperationKind::Call
            } else {
                return None;
            };
            Some(Operation {
                id: logical_id("operation", [function_id, &format!("source-{line_index}")]),
                package_id: package_id.clone(),
                function_id: function_id.to_string(),
                index_in_function: line_index,
                kind,
                display: trimmed.to_string(),
                target: extract_source_call_target(trimmed),
                source_span: source_span.clone(),
                metadata_json: Some(serde_json::json!({
                    "source_line": line_index + 1,
                    "precision": "source_heuristic"
                })),
            })
        })
        .collect()
}

fn extract_source_call_target(line: &str) -> Option<String> {
    let before_paren = line.split('(').next()?.trim();
    let candidate = before_paren
        .split_whitespace()
        .last()
        .unwrap_or(before_paren)
        .trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && character != '_' && character != ':'
        });
    candidate.contains("::").then(|| candidate.to_string())
}
