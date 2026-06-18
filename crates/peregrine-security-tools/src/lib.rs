use peregrine_types::{
    AuditCapabilityBinding, AuditProfile, AuditStageId, AuditTarget, AuditWorkItem,
    AuditWorkItemStatus, ExploitBundle, ExploitIntent, Metadata, ToolDiagnostic,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    future::Future,
    path::{Component, Path, PathBuf},
    pin::Pin,
    sync::Arc,
};
use thiserror::Error;

pub type AdapterFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, AuditAdapterError>> + Send + 'a>>;

pub fn default_audit_stages() -> Vec<AuditStageId> {
    vec![
        AuditStageId::BuildNormalize,
        AuditStageId::SemanticGraphs,
        AuditStageId::AttackSurface,
        AuditStageId::Invariants,
        AuditStageId::AttackHypotheses,
        AuditStageId::VerificationPlanning,
        AuditStageId::TargetedTests,
        AuditStageId::DynamicAnalysis,
        AuditStageId::SymbolicExecution,
        AuditStageId::EconomicSimulation,
        AuditStageId::AdversarialReview,
        AuditStageId::EvidenceAggregation,
        AuditStageId::FindingValidation,
        AuditStageId::SeverityRanking,
        AuditStageId::AuditReport,
        AuditStageId::AuditTrace,
    ]
}

pub fn create_audit_work_items(
    audit_id: &str,
    stages: &[AuditStageId],
    created_at: i64,
) -> Vec<AuditWorkItem> {
    stages
        .iter()
        .enumerate()
        .map(|(index, stage)| AuditWorkItem {
            id: format!("{audit_id}:stage:{index}"),
            stage: stage.clone(),
            status: AuditWorkItemStatus::Pending,
            title: format!("Complete {stage:?} stage"),
            claimed_by: None,
            attempts: 0,
            evidence_refs: Vec::new(),
            created_at,
            updated_at: created_at,
            metadata: Metadata::new(),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditWorkspace {
    pub root: PathBuf,
    pub input: PathBuf,
    pub workspace: PathBuf,
    pub artifacts: PathBuf,
    pub evidence: PathBuf,
    pub traces: PathBuf,
    pub reports: PathBuf,
}

impl AuditWorkspace {
    pub fn create(audits_root: &Path, audit_id: &str) -> Result<Self, AuditAdapterError> {
        let mut components = Path::new(audit_id).components();
        if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
            return Err(AuditAdapterError::InvalidTarget(
                "audit ID must be one path component".to_string(),
            ));
        }
        let root = audits_root.join(audit_id);
        let value = Self {
            input: root.join("input"),
            workspace: root.join("workspace"),
            artifacts: root.join("artifacts"),
            evidence: root.join("evidence"),
            traces: root.join("traces"),
            reports: root.join("reports"),
            root,
        };
        for directory in [
            &value.input,
            &value.workspace,
            &value.artifacts,
            &value.evidence,
            &value.traces,
            &value.reports,
        ] {
            std::fs::create_dir_all(directory).map_err(|source| AuditAdapterError::Io {
                action: "create audit workspace",
                source,
            })?;
        }
        Ok(value)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AuditTargetPreflight {
    pub adapter_id: String,
    pub normalized_target: AuditTarget,
    pub capabilities: Vec<AuditCapabilityBinding>,
    pub diagnostics: Vec<ToolDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcquiredAuditTarget {
    pub adapter_id: String,
    pub root: PathBuf,
    pub manifest_ref: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_refs: Vec<String>,
    pub immutable_state_ref: Option<String>,
    pub diagnostics: Vec<ToolDiagnostic>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ExploitReplay {
    pub bundle_id: String,
    pub succeeded: bool,
    pub evidence_refs: Vec<String>,
    pub diagnostics: Vec<ToolDiagnostic>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

/// Blockchain adapter used by the neutral audit coordinator.
///
/// Implementations own chain-specific target formats, imports, exploit encoding,
/// replay, and state hydration. They must only write beneath the supplied audit
/// workspace.
pub trait AuditChainAdapter: Send + Sync {
    fn adapter_id(&self) -> &'static str;
    fn chain_id(&self) -> &'static str;
    fn capabilities(&self) -> Vec<AuditCapabilityBinding>;
    fn preflight<'a>(&'a self, target: &'a AuditTarget) -> AdapterFuture<'a, AuditTargetPreflight>;
    fn acquire<'a>(
        &'a self,
        target: &'a AuditTarget,
        profile: &'a AuditProfile,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, AcquiredAuditTarget>;
    fn encode_exploit<'a>(
        &'a self,
        target: &'a AcquiredAuditTarget,
        intent: &'a ExploitIntent,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitBundle>;
    fn replay_exploit<'a>(
        &'a self,
        target: &'a AcquiredAuditTarget,
        bundle: &'a ExploitBundle,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitReplay>;
}

#[derive(Default)]
pub struct AuditAdapterRegistry {
    adapters: BTreeMap<String, Arc<dyn AuditChainAdapter>>,
}

impl AuditAdapterRegistry {
    pub fn register(&mut self, adapter: Arc<dyn AuditChainAdapter>) {
        self.adapters
            .insert(adapter.chain_id().to_string(), adapter);
    }

    pub fn get(&self, chain_id: &str) -> Result<Arc<dyn AuditChainAdapter>, AuditAdapterError> {
        self.adapters
            .get(chain_id)
            .cloned()
            .ok_or_else(|| AuditAdapterError::UnsupportedChain(chain_id.to_string()))
    }

    pub fn chain_ids(&self) -> Vec<String> {
        self.adapters.keys().cloned().collect()
    }
}

#[derive(Debug, Error)]
pub enum AuditAdapterError {
    #[error("unsupported audit chain `{0}`")]
    UnsupportedChain(String),
    #[error("invalid audit target: {0}")]
    InvalidTarget(String),
    #[error("audit adapter failed: {0}")]
    Adapter(String),
    #[error("io error while {action}: {source}")]
    Io {
        action: &'static str,
        #[source]
        source: std::io::Error,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_rejects_path_like_audit_ids() {
        let root = tempfile::tempdir().expect("tempdir");

        let error = AuditWorkspace::create(root.path(), "../outside").expect_err("invalid ID");

        assert!(matches!(error, AuditAdapterError::InvalidTarget(_)));
    }
}
