use super::{AuditStore, AuditStoreError, AuditStoreEvent, status_name};
use peregrine_types::{
    AuditAgentAssignment, AuditAgentAssignmentStatus, AuditAgentConclusion, AuditEvidence,
    AuditRun, AuditRunStatus, AuditStageId, AuditStageStatus, AuditWorkItem, AuditWorkItemStatus,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path};
use uuid::Uuid;

const MAX_ARTIFACT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkUpdate {
    pub run: AuditRun,
    pub work_item: AuditWorkItem,
    pub stage_changed: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AgentAssignmentUpdate {
    pub run: AuditRun,
    pub assignment: AuditAgentAssignment,
}

impl AuditStore {
    pub fn claim_work(
        &self,
        run_id: &str,
        worker_id: &str,
        stage: Option<&AuditStageId>,
        now: i64,
    ) -> Result<Option<(AuditRun, AuditWorkItem)>, AuditStoreError> {
        let claimed = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let Some(work_item) = run.work_items.iter_mut().find(|item| {
                item.status == AuditWorkItemStatus::Pending
                    && stage.is_none_or(|stage| &item.stage == stage)
            }) else {
                return Ok(None);
            };
            work_item.status = AuditWorkItemStatus::Claimed;
            work_item.claimed_by = Some(worker_id.to_string());
            work_item.attempts += 1;
            work_item.updated_at = now;
            run.current_stage = work_item.stage.clone();
            run.updated_at = now;
            Ok(Some(work_item.clone()))
        })?;
        if let (run, Some(work_item)) = &claimed {
            self.publish_event(AuditStoreEvent::StageUpdated {
                audit_id: run.id.clone(),
                stage: work_item.stage.clone(),
                status: AuditStageStatus::Running,
                run: run.clone(),
            });
        }
        Ok(claimed.1.map(|work_item| (claimed.0, work_item)))
    }

    pub fn record_packet(
        &self,
        run_id: &str,
        work_item_id: &str,
        packet_kind: &str,
        summary: &str,
        packet: Value,
        now: i64,
    ) -> Result<(AuditRun, String), AuditStoreError> {
        let artifact_id = Uuid::new_v4().to_string();
        let artifact_ref = format!("artifacts/{artifact_id}.json");
        let envelope = json!({
            "schemaVersion": 1,
            "id": artifact_id,
            "auditRunId": run_id,
            "workItemId": work_item_id,
            "kind": packet_kind,
            "summary": summary,
            "createdAt": now,
            "packet": packet,
        });
        self.mutate_run(run_id, |run| {
            ensure_claimed_work_item(run, work_item_id)?;
            self.write_json(
                &self.audits_root.join(run_id).join(&artifact_ref),
                &envelope,
            )?;
            run.artifact_refs.push(artifact_ref.clone());
            run.updated_at = now;
            Ok(artifact_ref.clone())
        })
    }

    pub fn record_evidence(
        &self,
        run_id: &str,
        mut evidence: AuditEvidence,
    ) -> Result<(AuditRun, String), AuditStoreError> {
        evidence.id = Uuid::new_v4().to_string();
        evidence.audit_run_id = run_id.to_string();
        let evidence_ref = format!("evidence/{}.json", evidence.id);
        self.mutate_run(run_id, |run| {
            if let Some(work_item_id) = evidence.work_item_id.as_deref() {
                ensure_claimed_work_item(run, work_item_id)?
                    .evidence_refs
                    .push(evidence_ref.clone());
            }
            self.write_json(
                &self.audits_root.join(run_id).join(&evidence_ref),
                &evidence,
            )?;
            run.evidence_refs.push(evidence_ref.clone());
            run.updated_at = evidence.created_at;
            Ok(evidence_ref.clone())
        })
    }

    pub fn record_agent_conclusion(
        &self,
        run_id: &str,
        work_item_id: &str,
        mut conclusion: AuditAgentConclusion,
    ) -> Result<(AuditRun, String), AuditStoreError> {
        conclusion.id = Uuid::new_v4().to_string();
        conclusion.audit_run_id = run_id.to_string();
        conclusion.work_item_id = work_item_id.to_string();
        let artifact_ref = format!("artifacts/agent-conclusions/{}.json", conclusion.id);
        let artifact_dir = self
            .audits_root
            .join(run_id)
            .join("artifacts/agent-conclusions");
        fs::create_dir_all(&artifact_dir).map_err(|source| AuditStoreError::Io {
            action: "create agent conclusion artifact directory",
            source,
        })?;
        self.mutate_run(run_id, |run| {
            ensure_claimed_work_item(run, work_item_id)?;
            for evidence_ref in &conclusion.evidence_refs {
                if !run.evidence_refs.contains(evidence_ref) {
                    return Err(AuditStoreError::EvidenceNotFound(evidence_ref.clone()));
                }
            }
            for artifact_ref in &conclusion.artifact_refs {
                if !run.artifact_refs.contains(artifact_ref) {
                    return Err(AuditStoreError::InvalidArtifactPath);
                }
            }
            self.write_json(
                &self.audits_root.join(run_id).join(&artifact_ref),
                &conclusion,
            )?;
            run.artifact_refs.push(artifact_ref.clone());
            if let Some(assignment) = run.agent_assignments.iter_mut().find(|assignment| {
                assignment.work_item_id == work_item_id
                    && assignment.role == conclusion.role
                    && (assignment.agent_thread_id.is_none()
                        || conclusion.agent_thread_id.is_none()
                        || assignment.agent_thread_id == conclusion.agent_thread_id)
            }) {
                if conclusion.agent_thread_id.is_some() {
                    assignment.agent_thread_id = conclusion.agent_thread_id.clone();
                }
                assignment.status = AuditAgentAssignmentStatus::Completed;
                assignment.conclusion_refs.push(artifact_ref.clone());
                assignment.updated_at = conclusion.created_at;
            }
            run.updated_at = conclusion.created_at;
            Ok(artifact_ref.clone())
        })
    }

    pub fn claim_agent_assignment(
        &self,
        run_id: &str,
        work_item_id: &str,
        assignment_id: &str,
        worker_id: &str,
        now: i64,
    ) -> Result<AgentAssignmentUpdate, AuditStoreError> {
        let (run, assignment) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let work_item = ensure_claimed_work_item(run, work_item_id)?;
            if work_item.claimed_by.as_deref() != Some(worker_id) {
                return Err(AuditStoreError::WorkItemClaimedByOther {
                    work_item_id: work_item_id.to_string(),
                    claimed_by: work_item
                        .claimed_by
                        .clone()
                        .unwrap_or_else(|| "unclaimed".to_string()),
                });
            }
            let assignment = ensure_agent_assignment(run, work_item_id, assignment_id)?;
            if assignment.status != AuditAgentAssignmentStatus::Pending {
                return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                    assignment_id: assignment_id.to_string(),
                    status: format!("{:?}", assignment.status),
                });
            }
            assignment.status = AuditAgentAssignmentStatus::Spawned;
            assignment.updated_at = now;
            let assignment = assignment.clone();
            run.updated_at = now;
            Ok(assignment)
        })?;
        Ok(AgentAssignmentUpdate { run, assignment })
    }

    pub fn update_agent_assignment_thread(
        &self,
        run_id: &str,
        work_item_id: &str,
        assignment_id: &str,
        agent_thread_id: &str,
        now: i64,
    ) -> Result<AgentAssignmentUpdate, AuditStoreError> {
        let (run, assignment) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            ensure_claimed_work_item(run, work_item_id)?;
            let assignment = ensure_agent_assignment(run, work_item_id, assignment_id)?;
            if assignment.status != AuditAgentAssignmentStatus::Spawned {
                return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                    assignment_id: assignment_id.to_string(),
                    status: format!("{:?}", assignment.status),
                });
            }
            assignment.agent_thread_id = Some(agent_thread_id.to_string());
            assignment.updated_at = now;
            let assignment = assignment.clone();
            run.updated_at = now;
            Ok(assignment)
        })?;
        Ok(AgentAssignmentUpdate { run, assignment })
    }

    pub fn finish_agent_assignment(
        &self,
        run_id: &str,
        work_item_id: &str,
        assignment_id: &str,
        status: AuditAgentAssignmentStatus,
        now: i64,
    ) -> Result<AgentAssignmentUpdate, AuditStoreError> {
        if !matches!(
            status,
            AuditAgentAssignmentStatus::Failed | AuditAgentAssignmentStatus::Cancelled
        ) {
            return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                assignment_id: assignment_id.to_string(),
                status: format!("{status:?}"),
            });
        }
        let (run, assignment) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            ensure_claimed_work_item(run, work_item_id)?;
            let assignment = ensure_agent_assignment(run, work_item_id, assignment_id)?;
            if matches!(assignment.status, AuditAgentAssignmentStatus::Completed) {
                return Err(AuditStoreError::InvalidAgentAssignmentStatus {
                    assignment_id: assignment_id.to_string(),
                    status: format!("{:?}", assignment.status),
                });
            }
            assignment.status = status;
            assignment.updated_at = now;
            let assignment = assignment.clone();
            run.updated_at = now;
            Ok(assignment)
        })?;
        Ok(AgentAssignmentUpdate { run, assignment })
    }

    pub fn record_router_evidence_for_current_work(
        &self,
        run_id: &str,
        mut evidence: AuditEvidence,
    ) -> Result<Option<(AuditRun, String)>, AuditStoreError> {
        evidence.id = Uuid::new_v4().to_string();
        let evidence_ref = format!("evidence/{}.json", evidence.id);
        self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let Some(work_item_index) = current_claimed_work_item_index(run) else {
                return Ok(None);
            };
            evidence.audit_run_id = run.id.clone();
            if evidence.adapter_id.is_none() {
                evidence.adapter_id = run.adapter_id.clone();
            }
            evidence.work_item_id = Some(run.work_items[work_item_index].id.clone());
            self.write_json(
                &self.audits_root.join(run_id).join(&evidence_ref),
                &evidence,
            )?;
            run.work_items[work_item_index]
                .evidence_refs
                .push(evidence_ref.clone());
            run.evidence_refs.push(evidence_ref.clone());
            run.updated_at = evidence.created_at;
            Ok(Some(evidence_ref.clone()))
        })
        .map(|(run, evidence_ref)| evidence_ref.map(|evidence_ref| (run, evidence_ref)))
    }

    pub fn finish_work(
        &self,
        run_id: &str,
        work_item_id: &str,
        worker_id: &str,
        status: AuditWorkItemStatus,
        evidence_refs: &[String],
        now: i64,
    ) -> Result<WorkUpdate, AuditStoreError> {
        if !matches!(
            status,
            AuditWorkItemStatus::Completed
                | AuditWorkItemStatus::Failed
                | AuditWorkItemStatus::Blocked
        ) {
            return Err(AuditStoreError::InvalidWorkStatus(format!("{status:?}")));
        }
        let (run, (work_item, previous_stage)) = self.mutate_run(run_id, |run| {
            ensure_running(run)?;
            let previous_stage = run.current_stage.clone();
            for evidence_ref in evidence_refs {
                if !run.evidence_refs.contains(evidence_ref) {
                    return Err(AuditStoreError::EvidenceNotFound(evidence_ref.clone()));
                }
            }
            ensure_agent_assignments_allow_work_status(run, work_item_id, &status)?;
            let work_item = ensure_work_item(run, work_item_id)?;
            if work_item.claimed_by.as_deref() != Some(worker_id) {
                return Err(AuditStoreError::WorkItemClaimedByOther {
                    work_item_id: work_item_id.to_string(),
                    claimed_by: work_item
                        .claimed_by
                        .clone()
                        .unwrap_or_else(|| "unclaimed".to_string()),
                });
            }
            work_item.status = status;
            work_item.updated_at = now;
            for evidence_ref in evidence_refs {
                if !work_item.evidence_refs.contains(evidence_ref) {
                    work_item.evidence_refs.push(evidence_ref.clone());
                }
            }
            let completed = work_item.clone();
            if let Some(next) = run
                .work_items
                .iter()
                .find(|item| item.status == AuditWorkItemStatus::Pending)
            {
                run.current_stage = next.stage.clone();
            }
            run.updated_at = now;
            Ok((completed, previous_stage))
        })?;
        self.publish_event(AuditStoreEvent::StageUpdated {
            audit_id: run.id.clone(),
            stage: work_item.stage.clone(),
            status: work_item_stage_status(&work_item.status),
            run: run.clone(),
        });
        Ok(WorkUpdate {
            stage_changed: previous_stage != run.current_stage,
            run,
            work_item,
        })
    }

    pub(crate) fn mutate_run<T>(
        &self,
        run_id: &str,
        mutate: impl FnOnce(&mut AuditRun) -> Result<T, AuditStoreError>,
    ) -> Result<(AuditRun, T), AuditStoreError> {
        let mut connection = self.connection()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let body = transaction
            .query_row(
                "SELECT body_json FROM audit_runs WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| AuditStoreError::RunNotFound(run_id.to_string()))?;
        let mut run: AuditRun = serde_json::from_str(&body)?;
        let result = mutate(&mut run)?;
        let body_json = serde_json::to_string(&run)?;
        transaction.execute(
            "
            UPDATE audit_runs
            SET status = ?2, updated_at = ?3, body_json = ?4
            WHERE run_id = ?1
            ",
            params![run.id, status_name(&run)?, run.updated_at, body_json],
        )?;
        transaction.commit()?;
        Ok((run, result))
    }

    pub(crate) fn write_json(
        &self,
        path: &Path,
        value: &impl Serialize,
    ) -> Result<(), AuditStoreError> {
        let bytes = serde_json::to_vec_pretty(value)?;
        self.write_bytes(path, &bytes)
    }

    pub(crate) fn write_text(&self, path: &Path, value: &str) -> Result<(), AuditStoreError> {
        self.write_bytes(path, value.as_bytes())
    }

    pub fn read_artifact(
        &self,
        run_id: &str,
        artifact_ref: &str,
    ) -> Result<Vec<u8>, AuditStoreError> {
        validate_artifact_ref(artifact_ref)?;
        let audit_root = self
            .audits_root
            .join(run_id)
            .canonicalize()
            .map_err(|source| AuditStoreError::Io {
                action: "resolve audit root",
                source,
            })?;
        let artifact_path = audit_root.join(artifact_ref);
        let canonical_artifact =
            artifact_path
                .canonicalize()
                .map_err(|source| AuditStoreError::Io {
                    action: "resolve audit artifact",
                    source,
                })?;
        if !canonical_artifact.starts_with(&audit_root) || !canonical_artifact.is_file() {
            return Err(AuditStoreError::InvalidArtifactPath);
        }
        let metadata = fs::metadata(&canonical_artifact).map_err(|source| AuditStoreError::Io {
            action: "inspect audit artifact",
            source,
        })?;
        if metadata.len() > MAX_ARTIFACT_BYTES as u64 {
            return Err(AuditStoreError::ArtifactTooLarge(MAX_ARTIFACT_BYTES));
        }
        fs::read(&canonical_artifact).map_err(|source| AuditStoreError::Io {
            action: "read audit artifact",
            source,
        })
    }

    fn write_bytes(&self, path: &Path, bytes: &[u8]) -> Result<(), AuditStoreError> {
        if bytes.len() > MAX_ARTIFACT_BYTES {
            return Err(AuditStoreError::ArtifactTooLarge(MAX_ARTIFACT_BYTES));
        }
        let audit_root = path
            .parent()
            .and_then(Path::parent)
            .ok_or(AuditStoreError::InvalidArtifactPath)?
            .canonicalize()
            .map_err(|source| AuditStoreError::Io {
                action: "resolve audit root",
                source,
            })?;
        let parent = path
            .parent()
            .ok_or(AuditStoreError::InvalidArtifactPath)?
            .canonicalize()
            .map_err(|source| AuditStoreError::Io {
                action: "resolve audit artifact directory",
                source,
            })?;
        if !parent.starts_with(&audit_root) || path.exists() {
            return Err(AuditStoreError::InvalidArtifactPath);
        }
        let temporary = path.with_extension("json.tmp");
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|source| AuditStoreError::Io {
                action: "create audit artifact",
                source,
            })?;
        file.write_all(&bytes)
            .map_err(|source| AuditStoreError::Io {
                action: "write audit artifact",
                source,
            })?;
        file.sync_all().map_err(|source| AuditStoreError::Io {
            action: "sync audit artifact",
            source,
        })?;
        std::fs::rename(&temporary, path).map_err(|source| AuditStoreError::Io {
            action: "commit audit artifact",
            source,
        })
    }
}

fn validate_artifact_ref(artifact_ref: &str) -> Result<(), AuditStoreError> {
    let path = Path::new(artifact_ref);
    if artifact_ref.is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(AuditStoreError::InvalidArtifactPath);
    }
    Ok(())
}

fn work_item_stage_status(status: &AuditWorkItemStatus) -> AuditStageStatus {
    match status {
        AuditWorkItemStatus::Pending => AuditStageStatus::Pending,
        AuditWorkItemStatus::Claimed => AuditStageStatus::Running,
        AuditWorkItemStatus::Completed => AuditStageStatus::Succeeded,
        AuditWorkItemStatus::Failed => AuditStageStatus::Failed,
        AuditWorkItemStatus::Blocked => AuditStageStatus::Blocked,
        AuditWorkItemStatus::Cancelled => AuditStageStatus::Cancelled,
    }
}

fn ensure_running(run: &AuditRun) -> Result<(), AuditStoreError> {
    if run.status == AuditRunStatus::Running {
        Ok(())
    } else {
        Err(AuditStoreError::InvalidRunState {
            run_id: run.id.clone(),
            status: format!("{:?}", run.status),
        })
    }
}

fn ensure_work_item<'a>(
    run: &'a mut AuditRun,
    work_item_id: &str,
) -> Result<&'a mut AuditWorkItem, AuditStoreError> {
    run.work_items
        .iter_mut()
        .find(|item| item.id == work_item_id)
        .ok_or_else(|| AuditStoreError::WorkItemNotFound(work_item_id.to_string()))
}

fn ensure_claimed_work_item<'a>(
    run: &'a mut AuditRun,
    work_item_id: &str,
) -> Result<&'a mut AuditWorkItem, AuditStoreError> {
    let work_item = ensure_work_item(run, work_item_id)?;
    if work_item.status != AuditWorkItemStatus::Claimed {
        return Err(AuditStoreError::WorkItemNotClaimed(
            work_item_id.to_string(),
        ));
    }
    Ok(work_item)
}

fn ensure_agent_assignment<'a>(
    run: &'a mut AuditRun,
    work_item_id: &str,
    assignment_id: &str,
) -> Result<&'a mut AuditAgentAssignment, AuditStoreError> {
    run.agent_assignments
        .iter_mut()
        .find(|assignment| {
            assignment.id == assignment_id && assignment.work_item_id == work_item_id
        })
        .ok_or_else(|| AuditStoreError::AgentAssignmentNotFound(assignment_id.to_string()))
}

fn ensure_agent_assignments_allow_work_status(
    run: &AuditRun,
    work_item_id: &str,
    status: &AuditWorkItemStatus,
) -> Result<(), AuditStoreError> {
    if *status != AuditWorkItemStatus::Completed {
        return Ok(());
    }
    let assignments = run
        .agent_assignments
        .iter()
        .filter(|assignment| assignment.work_item_id == work_item_id)
        .collect::<Vec<_>>();
    let incomplete = assignments
        .iter()
        .filter(|assignment| {
            matches!(
                assignment.status,
                AuditAgentAssignmentStatus::Pending | AuditAgentAssignmentStatus::Spawned
            )
        })
        .map(|assignment| assignment.id.clone())
        .collect::<Vec<_>>();
    if !incomplete.is_empty() {
        return Err(AuditStoreError::IncompleteAgentAssignments {
            work_item_id: work_item_id.to_string(),
            assignment_ids: incomplete,
        });
    }
    let failed = assignments
        .iter()
        .filter(|assignment| {
            matches!(
                assignment.status,
                AuditAgentAssignmentStatus::Failed | AuditAgentAssignmentStatus::Cancelled
            )
        })
        .map(|assignment| assignment.id.clone())
        .collect::<Vec<_>>();
    if !failed.is_empty() {
        return Err(AuditStoreError::FailedAgentAssignments {
            work_item_id: work_item_id.to_string(),
            assignment_ids: failed,
        });
    }
    Ok(())
}

fn current_claimed_work_item_index(run: &AuditRun) -> Option<usize> {
    run.work_items
        .iter()
        .position(|item| {
            item.status == AuditWorkItemStatus::Claimed && item.stage == run.current_stage
        })
        .or_else(|| {
            run.work_items
                .iter()
                .position(|item| item.status == AuditWorkItemStatus::Claimed)
        })
}
