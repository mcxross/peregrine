use super::{AuditStore, AuditStoreError, status_name};
use peregrine_types::{
    AuditEvidence, AuditRun, AuditRunStatus, AuditStageId, AuditWorkItem, AuditWorkItemStatus,
};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use serde::Serialize;
use serde_json::{Value, json};
use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;
use uuid::Uuid;

const MAX_ARTIFACT_BYTES: usize = 512 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct WorkUpdate {
    pub run: AuditRun,
    pub work_item: AuditWorkItem,
    pub stage_changed: bool,
}

impl AuditStore {
    pub fn claim_work(
        &self,
        run_id: &str,
        worker_id: &str,
        stage: Option<&AuditStageId>,
        now: i64,
    ) -> Result<Option<(AuditRun, AuditWorkItem)>, AuditStoreError> {
        self.mutate_run(run_id, |run| {
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
        })
        .map(|(run, work_item)| work_item.map(|work_item| (run, work_item)))
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
        Ok(WorkUpdate {
            stage_changed: previous_stage != run.current_stage,
            run,
            work_item,
        })
    }

    fn mutate_run<T>(
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

    fn write_json(&self, path: &Path, value: &impl Serialize) -> Result<(), AuditStoreError> {
        let bytes = serde_json::to_vec_pretty(value)?;
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
