use crate::core::{stable_id, Edge, EdgeType, OperationId, PackageId, SourceSpan};

pub fn call_edge(
    package_id: &PackageId,
    caller_id: &str,
    callee_id_or_name: &str,
    operation_id: Option<OperationId>,
    source_span: SourceSpan,
) -> Edge {
    Edge {
        id: stable_id(
            "edge",
            [
                package_id.as_str(),
                caller_id,
                callee_id_or_name,
                operation_id.as_deref().unwrap_or("_"),
                "CALLS",
            ],
        ),
        package_id: package_id.clone(),
        from_id: caller_id.to_string(),
        to_id: callee_id_or_name.to_string(),
        edge_type: EdgeType::Calls,
        operation_id,
        source_span,
        metadata_json: None,
    }
}
