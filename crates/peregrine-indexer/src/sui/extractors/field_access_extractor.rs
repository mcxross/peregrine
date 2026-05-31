use crate::core::{Edge, EdgeType, OperationId, PackageId, SourceSpan, stable_id};

pub fn field_access_edge(
    package_id: &PackageId,
    function_id: &str,
    field_id_or_name: &str,
    access_kind: EdgeType,
    operation_id: Option<OperationId>,
    source_span: SourceSpan,
) -> Edge {
    let kind = format!("{access_kind:?}");
    Edge {
        id: stable_id(
            "edge",
            [
                package_id.as_str(),
                function_id,
                field_id_or_name,
                operation_id.as_deref().unwrap_or("_"),
                &kind,
            ],
        ),
        package_id: package_id.clone(),
        from_id: function_id.to_string(),
        to_id: field_id_or_name.to_string(),
        edge_type: access_kind,
        operation_id,
        source_span,
        metadata_json: None,
    }
}
