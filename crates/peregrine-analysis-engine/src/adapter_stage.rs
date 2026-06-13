use peregrine_analysis::{
    AnalysisError, AnalysisRequest, AnalysisTarget, ChainAdapter, ResolvedTarget,
};
use serde_json::{Value, json};

pub(crate) async fn prepare_target(
    adapter: &dyn ChainAdapter,
    request: &AnalysisRequest,
    mut resolved: ResolvedTarget,
) -> Result<ResolvedTarget, AnalysisError> {
    let metadata = adapter.normalize_metadata(resolved.metadata)?;
    resolved.metadata = metadata;

    match &request.target {
        AnalysisTarget::LocalPackage { .. } | AnalysisTarget::OnChainPackage { .. } => {
            let package = adapter.retrieve_package(&resolved, &request.limits).await?;
            let dependencies = adapter
                .resolve_dependencies(&package, &request.limits)
                .await?;
            insert_metadata(
                &mut resolved.metadata,
                "adapterPackage",
                json!({
                    "id": package.id,
                    "root": package.root,
                    "byteLength": package.bytes.len(),
                    "metadata": adapter.normalize_metadata(package.metadata)?,
                    "dependencies": dependencies.into_iter().map(|dependency| json!({
                        "id": dependency.id,
                        "root": dependency.root,
                        "byteLength": dependency.bytes.len(),
                        "metadata": dependency.metadata,
                    })).collect::<Vec<_>>(),
                }),
            )?;
        }
        AnalysisTarget::Transaction { .. } => {
            let transaction = adapter
                .retrieve_transaction(&resolved, &request.limits)
                .await?;
            insert_metadata(
                &mut resolved.metadata,
                "adapterTransaction",
                json!({
                    "digest": transaction.digest,
                    "byteLength": transaction.bytes.len(),
                    "metadata": adapter.normalize_metadata(transaction.metadata)?,
                }),
            )?;
        }
    }

    Ok(resolved)
}

fn insert_metadata(metadata: &mut Value, key: &str, value: Value) -> Result<(), AnalysisError> {
    let object = metadata.as_object_mut().ok_or_else(|| {
        AnalysisError::new(
            "invalid_metadata",
            "normalized adapter metadata must be a JSON object",
        )
    })?;
    object.insert(key.to_string(), value);
    Ok(())
}
