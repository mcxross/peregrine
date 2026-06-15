use peregrine_security_tools::AuditWorkspace;
use peregrine_types::{AuditPlan, AuditRun};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use thiserror::Error;

mod operations;

pub use operations::WorkUpdate;

const DATABASE_FILE: &str = "audits.sqlite";

pub struct AuditStore {
    audits_root: PathBuf,
    connection: Mutex<Connection>,
}

impl AuditStore {
    pub fn open(peregrine_home: &Path) -> Result<Self, AuditStoreError> {
        let audits_root = peregrine_home.join("audits");
        std::fs::create_dir_all(&audits_root).map_err(|source| AuditStoreError::Io {
            action: "create audits directory",
            source,
        })?;
        let connection = Connection::open(audits_root.join(DATABASE_FILE))?;
        connection.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;

            CREATE TABLE IF NOT EXISTS audit_plans (
                fingerprint TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                body_json TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS audit_runs (
                run_id TEXT PRIMARY KEY,
                plan_fingerprint TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                body_json TEXT NOT NULL,
                FOREIGN KEY(plan_fingerprint) REFERENCES audit_plans(fingerprint)
            );

            CREATE INDEX IF NOT EXISTS audit_runs_updated_at
                ON audit_runs(updated_at DESC);
            ",
        )?;
        Ok(Self {
            audits_root,
            connection: Mutex::new(connection),
        })
    }

    pub fn audits_root(&self) -> &Path {
        &self.audits_root
    }

    pub fn create_workspace(&self, audit_id: &str) -> Result<AuditWorkspace, AuditStoreError> {
        AuditWorkspace::create(&self.audits_root, audit_id).map_err(AuditStoreError::Adapter)
    }

    pub fn store_plan(&self, mut plan: AuditPlan) -> Result<AuditPlan, AuditStoreError> {
        plan.fingerprint = fingerprint_plan(&plan)?;
        let body_json = serde_json::to_string(&plan)?;
        self.connection()?
            .execute(
                "
                INSERT INTO audit_plans (fingerprint, plan_id, created_at, body_json)
                VALUES (?1, ?2, ?3, ?4)
                ON CONFLICT(fingerprint) DO UPDATE SET body_json = excluded.body_json
                ",
                params![plan.fingerprint, plan.id, plan.created_at, body_json],
            )
            .map(|_| plan)
            .map_err(AuditStoreError::from)
    }

    pub fn read_plan(&self, fingerprint: &str) -> Result<Option<AuditPlan>, AuditStoreError> {
        let body = self
            .connection()?
            .query_row(
                "SELECT body_json FROM audit_plans WHERE fingerprint = ?1",
                [fingerprint],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        body.map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(AuditStoreError::from)
    }

    pub fn create_run(&self, run: &AuditRun) -> Result<(), AuditStoreError> {
        self.create_workspace(&run.id)?;
        let body_json = serde_json::to_string(run)?;
        self.connection()?.execute(
            "
            INSERT INTO audit_runs (
                run_id, plan_fingerprint, status, created_at, updated_at, body_json
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ",
            params![
                run.id,
                run.plan_fingerprint,
                status_name(run)?,
                run.created_at,
                run.updated_at,
                body_json
            ],
        )?;
        Ok(())
    }

    pub fn update_run(&self, run: &AuditRun) -> Result<(), AuditStoreError> {
        let body_json = serde_json::to_string(run)?;
        let updated = self.connection()?.execute(
            "
            UPDATE audit_runs
            SET status = ?2, updated_at = ?3, body_json = ?4
            WHERE run_id = ?1
            ",
            params![run.id, status_name(run)?, run.updated_at, body_json],
        )?;
        if updated == 0 {
            return Err(AuditStoreError::RunNotFound(run.id.clone()));
        }
        Ok(())
    }

    pub fn read_run(&self, run_id: &str) -> Result<Option<AuditRun>, AuditStoreError> {
        let body = self
            .connection()?
            .query_row(
                "SELECT body_json FROM audit_runs WHERE run_id = ?1",
                [run_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        body.map(|value| serde_json::from_str(&value))
            .transpose()
            .map_err(AuditStoreError::from)
    }

    pub fn list_runs(&self, limit: u32) -> Result<Vec<AuditRun>, AuditStoreError> {
        let connection = self.connection()?;
        let mut statement = connection
            .prepare("SELECT body_json FROM audit_runs ORDER BY updated_at DESC LIMIT ?1")?;
        let rows = statement.query_map([i64::from(limit.clamp(1, 1_000))], |row| {
            row.get::<_, String>(0)
        })?;
        rows.map(|row| {
            let body = row?;
            serde_json::from_str(&body).map_err(AuditStoreError::from)
        })
        .collect()
    }

    pub fn delete_run(&self, run_id: &str) -> Result<bool, AuditStoreError> {
        let deleted = self
            .connection()?
            .execute("DELETE FROM audit_runs WHERE run_id = ?1", [run_id])?;
        if deleted == 0 {
            return Ok(false);
        }
        let workspace = self.audits_root.join(run_id);
        if workspace.exists() {
            std::fs::remove_dir_all(workspace).map_err(|source| AuditStoreError::Io {
                action: "delete audit workspace",
                source,
            })?;
        }
        Ok(true)
    }

    fn connection(&self) -> Result<std::sync::MutexGuard<'_, Connection>, AuditStoreError> {
        self.connection
            .lock()
            .map_err(|_| AuditStoreError::DatabaseLock)
    }
}

pub fn fingerprint_plan(plan: &AuditPlan) -> Result<String, AuditStoreError> {
    let mut canonical = plan.clone();
    canonical.fingerprint.clear();
    let bytes = serde_json::to_vec(&canonical)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn status_name(run: &AuditRun) -> Result<String, AuditStoreError> {
    serde_json::to_value(&run.status)?
        .as_str()
        .map(str::to_string)
        .ok_or(AuditStoreError::InvalidStatus)
}

#[derive(Debug, Error)]
pub enum AuditStoreError {
    #[error("audit database lock is poisoned")]
    DatabaseLock,
    #[error("audit run `{0}` was not found")]
    RunNotFound(String),
    #[error("audit work item `{0}` was not found")]
    WorkItemNotFound(String),
    #[error("audit work item `{work_item_id}` is claimed by `{claimed_by}`")]
    WorkItemClaimedByOther {
        work_item_id: String,
        claimed_by: String,
    },
    #[error("audit work item `{0}` is not currently claimed")]
    WorkItemNotClaimed(String),
    #[error("audit evidence reference `{0}` was not recorded")]
    EvidenceNotFound(String),
    #[error("audit run `{run_id}` is not mutable while {status}")]
    InvalidRunState { run_id: String, status: String },
    #[error("audit work item cannot transition to {0}")]
    InvalidWorkStatus(String),
    #[error("audit artifact exceeds the {0}-byte storage limit")]
    ArtifactTooLarge(usize),
    #[error("audit artifact path is outside its managed audit directory")]
    InvalidArtifactPath,
    #[error("audit status did not serialize as a string")]
    InvalidStatus,
    #[error(transparent)]
    Adapter(#[from] peregrine_security_tools::AuditAdapterError),
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error(transparent)]
    Serialization(#[from] serde_json::Error),
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
    use peregrine_security_tools::create_audit_work_items;
    use peregrine_types::{
        AuditEvidence, AuditEvidenceAttestation, AuditProfile, AuditRunStatus, AuditStageId,
        AuditTarget, AuditWorkItemStatus, Metadata, SourcePrecision, VerificationMethod,
    };

    fn plan() -> AuditPlan {
        AuditPlan {
            schema_version: 1,
            id: "plan-1".to_string(),
            fingerprint: String::new(),
            target: AuditTarget::LocalPackage {
                chain_id: "test".to_string(),
                path: "/tmp/package".to_string(),
                metadata: Metadata::new(),
            },
            profile: AuditProfile::default(),
            stages: vec![AuditStageId::AuditSession],
            required_capabilities: vec!["target.acquire".to_string()],
            created_at: 10,
            metadata: Metadata::new(),
        }
    }

    #[test]
    fn stores_plan_run_and_workspace() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let plan = store.store_plan(plan()).expect("store plan");
        let run = AuditRun {
            schema_version: 1,
            id: "audit-1".to_string(),
            plan_fingerprint: plan.fingerprint.clone(),
            target: plan.target.clone(),
            profile: plan.profile.clone(),
            status: AuditRunStatus::Pending,
            current_stage: AuditStageId::AuditSession,
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: None,
            capabilities: Vec::new(),
            coverage_gaps: Vec::new(),
            work_items: Vec::new(),
            evidence_refs: Vec::new(),
            artifact_refs: Vec::new(),
            created_at: 10,
            updated_at: 10,
            metadata: Metadata::new(),
        };

        store.create_run(&run).expect("create run");

        assert_eq!(
            store.read_run("audit-1").expect("read run"),
            Some(run.clone())
        );
        assert!(store.audits_root().join("audit-1/input").is_dir());
        assert_eq!(store.list_runs(10).expect("list runs"), vec![run]);
    }

    #[test]
    fn claims_records_and_finishes_work_atomically() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let plan = store.store_plan(plan()).expect("store plan");
        let stages = vec![AuditStageId::BuildNormalize, AuditStageId::AttackSurface];
        let run = AuditRun {
            schema_version: 1,
            id: "audit-work".to_string(),
            plan_fingerprint: plan.fingerprint,
            target: plan.target,
            profile: plan.profile,
            status: AuditRunStatus::Running,
            current_stage: stages[0].clone(),
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: Some("test".to_string()),
            capabilities: Vec::new(),
            coverage_gaps: Vec::new(),
            work_items: create_audit_work_items("audit-work", &stages, 10),
            evidence_refs: Vec::new(),
            artifact_refs: Vec::new(),
            created_at: 10,
            updated_at: 10,
            metadata: Metadata::new(),
        };
        store.create_run(&run).expect("create run");

        let (_, claimed) = store
            .claim_work("audit-work", "researcher", None, 11)
            .expect("claim work")
            .expect("pending work");
        let evidence = AuditEvidence {
            id: String::new(),
            audit_run_id: String::new(),
            work_item_id: Some(claimed.id.clone()),
            verification_method: VerificationMethod::StaticAnalysis,
            provider_id: "native".to_string(),
            adapter_id: Some("test".to_string()),
            tool_name: "test-analyzer".to_string(),
            tool_version: Some("1".to_string()),
            input_hash: "abc".to_string(),
            source_precision: SourcePrecision::SourceMap,
            attestation: AuditEvidenceAttestation::RouterCaptured,
            summary: "reachable state mutation".to_string(),
            observation: "analyzer produced a source-mapped path".to_string(),
            execution_trace_ref: None,
            artifact_refs: Vec::new(),
            created_at: 12,
            metadata: Metadata::new(),
        };
        let (_, evidence_ref) = store
            .record_evidence("audit-work", evidence)
            .expect("record evidence");
        let update = store
            .finish_work(
                "audit-work",
                &claimed.id,
                "researcher",
                AuditWorkItemStatus::Completed,
                &[evidence_ref],
                13,
            )
            .expect("finish work");

        assert!(update.stage_changed);
        assert_eq!(update.run.current_stage, AuditStageId::AttackSurface);
        assert_eq!(update.work_item.status, AuditWorkItemStatus::Completed);
    }

    #[test]
    fn fingerprint_tracks_immutable_plan_content() {
        let mut plan = plan();
        let original = fingerprint_plan(&plan).expect("fingerprint");
        plan.fingerprint = "ignored-existing-value".to_string();

        assert_eq!(fingerprint_plan(&plan).expect("fingerprint"), original);

        plan.profile.max_hypotheses += 1;
        assert_ne!(fingerprint_plan(&plan).expect("fingerprint"), original);
    }
}
