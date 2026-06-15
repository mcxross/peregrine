use super::ContextualUserFragment;
use peregrine_types::AuditRun;
use serde::Serialize;

const START_MARKER: &str = "<audit_run_context>";
const END_MARKER: &str = "</audit_run_context>";
const MAX_BODY_BYTES: usize = 8_000;
const MAX_REFS: usize = 12;

/// Bounded model context describing the active persisted audit run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRunContextFragment {
    body: String,
}

impl AuditRunContextFragment {
    pub fn from_run(run: &AuditRun) -> Self {
        let context = AuditRunContext {
            audit_id: &run.id,
            plan_fingerprint: &run.plan_fingerprint,
            status: format!("{:?}", run.status),
            current_stage: format!("{:?}", run.current_stage),
            model_token_budget: run.profile.model_token_budget,
            wall_time_seconds: run.profile.wall_time_seconds,
            max_hypotheses: run.profile.max_hypotheses,
            coverage_gaps: run
                .coverage_gaps
                .iter()
                .take(MAX_REFS)
                .map(|gap| format!("{}: {}", gap.capability, gap.reason))
                .collect(),
            evidence_refs: run.evidence_refs.iter().take(MAX_REFS).collect(),
            artifact_refs: run.artifact_refs.iter().take(MAX_REFS).collect(),
            work_items: run.work_items.len(),
        };
        let mut body = serde_json::to_string_pretty(&context)
            .unwrap_or_else(|error| format!("{{\"serializationError\":\"{error}\"}}"));
        if body.len() > MAX_BODY_BYTES {
            body.truncate(MAX_BODY_BYTES);
        }
        Self { body }
    }
}

impl ContextualUserFragment for AuditRunContextFragment {
    fn role() -> &'static str {
        "user"
    }

    fn markers(&self) -> (&'static str, &'static str) {
        Self::type_markers()
    }

    fn body(&self) -> String {
        format!(
            "\nThis is bounded audit state. Retrieve details through registered audit tools.\n{}\n",
            self.body
        )
    }

    fn type_markers() -> (&'static str, &'static str) {
        (START_MARKER, END_MARKER)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditRunContext<'a> {
    audit_id: &'a str,
    plan_fingerprint: &'a str,
    status: String,
    current_stage: String,
    model_token_budget: i64,
    wall_time_seconds: i64,
    max_hypotheses: u32,
    coverage_gaps: Vec<String>,
    evidence_refs: Vec<&'a String>,
    artifact_refs: Vec<&'a String>,
    work_items: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use peregrine_types::{AuditProfile, AuditRunStatus, AuditStageId, AuditTarget, Metadata};

    #[test]
    fn fragment_is_bounded() {
        let run = AuditRun {
            schema_version: 1,
            id: "audit-1".to_string(),
            plan_fingerprint: "fingerprint".to_string(),
            target: AuditTarget::LocalPackage {
                chain_id: "sui".to_string(),
                path: "/tmp/package".to_string(),
                metadata: Metadata::new(),
            },
            profile: AuditProfile::default(),
            status: AuditRunStatus::Running,
            current_stage: AuditStageId::BuildNormalize,
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: None,
            capabilities: Vec::new(),
            coverage_gaps: Vec::new(),
            work_items: Vec::new(),
            evidence_refs: (0..100).map(|index| format!("evidence-{index}")).collect(),
            artifact_refs: (0..100).map(|index| format!("artifact-{index}")).collect(),
            created_at: 0,
            updated_at: 0,
            metadata: Metadata::new(),
        };

        let rendered = AuditRunContextFragment::from_run(&run).render();

        assert!(rendered.len() <= MAX_BODY_BYTES + 300);
        assert!(rendered.contains("audit-1"));
        assert!(!rendered.contains("evidence-99"));
    }
}
