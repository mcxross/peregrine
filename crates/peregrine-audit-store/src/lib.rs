use peregrine_security_tools::AuditWorkspace;
use peregrine_types::{AuditPlan, AuditRun, AuditStageId, AuditStageStatus, FindingCandidate};
use rusqlite::{Connection, OptionalExtension, params};
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{
        Mutex, OnceLock,
        mpsc::{self, Receiver, Sender},
    },
};
use thiserror::Error;

mod operations;
mod report;

pub use operations::WorkUpdate;
pub use report::FinalizedAuditReport;

const DATABASE_FILE: &str = "audits.sqlite";

static EVENT_BROKERS: OnceLock<Mutex<HashMap<PathBuf, Vec<Sender<AuditStoreEvent>>>>> =
    OnceLock::new();

#[derive(Debug, Clone, PartialEq)]
pub enum AuditStoreEvent {
    StageUpdated {
        audit_id: String,
        stage: AuditStageId,
        status: AuditStageStatus,
        run: AuditRun,
    },
    FindingUpdated {
        audit_id: String,
        finding: FindingCandidate,
        report_ref: String,
    },
}

pub struct AuditStore {
    audits_root: PathBuf,
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq)]
/// A bounded page of persisted audit runs ordered by newest update first.
pub struct AuditRunListPage {
    pub runs: Vec<AuditRun>,
    pub next_cursor: Option<String>,
}

impl AuditStore {
    pub fn open(peregrine_home: &Path) -> Result<Self, AuditStoreError> {
        let audits_root = peregrine_home.join("audits");
        std::fs::create_dir_all(&audits_root).map_err(|source| AuditStoreError::Io {
            action: "create audits directory",
            source,
        })?;
        let audits_root = audits_root
            .canonicalize()
            .map_err(|source| AuditStoreError::Io {
                action: "canonicalize audits directory",
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

    pub fn subscribe_events(&self) -> Result<Receiver<AuditStoreEvent>, AuditStoreError> {
        let (sender, receiver) = mpsc::channel();
        let brokers = EVENT_BROKERS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut brokers = brokers.lock().map_err(|_| AuditStoreError::EventBusLock)?;
        brokers
            .entry(self.audits_root.clone())
            .or_default()
            .push(sender);
        Ok(receiver)
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
        self.list_runs_page(/*offset*/ 0, limit)
            .map(|page| page.runs)
    }

    pub fn list_runs_page(
        &self,
        offset: u32,
        limit: u32,
    ) -> Result<AuditRunListPage, AuditStoreError> {
        let limit = limit.clamp(1, 1_000);
        let fetch_limit = limit + 1;
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "
            SELECT body_json FROM audit_runs
            ORDER BY updated_at DESC, run_id DESC
            LIMIT ?1 OFFSET ?2
            ",
        )?;
        let rows = statement
            .query_map(params![i64::from(fetch_limit), i64::from(offset)], |row| {
                row.get::<_, String>(0)
            })?;
        let mut runs = rows
            .map(|row| {
                let body = row?;
                serde_json::from_str(&body).map_err(AuditStoreError::from)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let next_cursor = if runs.len() > limit as usize {
            runs.truncate(limit as usize);
            Some(offset.saturating_add(limit).to_string())
        } else {
            None
        };
        Ok(AuditRunListPage { runs, next_cursor })
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

    pub(crate) fn publish_event(&self, event: AuditStoreEvent) {
        let Some(brokers) = EVENT_BROKERS.get() else {
            return;
        };
        let Ok(mut brokers) = brokers.lock() else {
            return;
        };
        let Some(senders) = brokers.get_mut(&self.audits_root) else {
            return;
        };
        senders.retain(|sender| sender.send(event.clone()).is_ok());
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
    #[error("audit event bus lock is poisoned")]
    EventBusLock,
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
    #[error("invalid audit report: {0}")]
    InvalidReport(String),
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
        AuditEvidence, AuditEvidenceAttestation, AuditProfile, AuditReport, AuditRunStatus,
        AuditStageId, AuditStageStatus, AuditTarget, AuditWorkItemStatus, EvidenceConfidence,
        FindingCandidate, FindingCandidateSeverity, FindingCandidateStatus, Metadata,
        SourcePrecision, ValidationPlan, VerificationMethod,
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
    fn list_runs_paginates_by_updated_at_then_id() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let plan = store.store_plan(plan()).expect("store plan");
        let mut expected = Vec::new();
        for (id, updated_at) in [
            ("audit-a", 10),
            ("audit-c", 20),
            ("audit-b", 20),
            ("audit-d", 5),
        ] {
            let run = AuditRun {
                schema_version: 1,
                id: id.to_string(),
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
                created_at: updated_at,
                updated_at,
                metadata: Metadata::new(),
            };
            store.create_run(&run).expect("create run");
            expected.push(run);
        }
        expected.sort_by(|left, right| {
            right
                .updated_at
                .cmp(&left.updated_at)
                .then_with(|| right.id.cmp(&left.id))
        });

        let first_page = store
            .list_runs_page(/*offset*/ 0, /*limit*/ 2)
            .expect("first page");
        assert_eq!(
            first_page,
            AuditRunListPage {
                runs: expected[..2].to_vec(),
                next_cursor: Some("2".to_string()),
            }
        );
        let second_page = store
            .list_runs_page(/*offset*/ 2, /*limit*/ 2)
            .expect("second page");
        assert_eq!(
            second_page,
            AuditRunListPage {
                runs: expected[2..].to_vec(),
                next_cursor: None,
            }
        );
    }

    #[test]
    fn read_artifact_stays_inside_audit_workspace() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let plan = store.store_plan(plan()).expect("store plan");
        let run = AuditRun {
            schema_version: 1,
            id: "audit-read".to_string(),
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
        let body: &[u8] = br#"{"ok":true}"#;
        std::fs::write(
            store
                .audits_root()
                .join("audit-read")
                .join("artifacts/example.json"),
            body,
        )
        .expect("write artifact");

        assert_eq!(
            store
                .read_artifact("audit-read", "artifacts/example.json")
                .expect("read artifact"),
            body
        );
        assert!(matches!(
            store.read_artifact("audit-read", "../audits.sqlite"),
            Err(AuditStoreError::InvalidArtifactPath)
        ));
        assert!(matches!(
            store.read_artifact("audit-read", "/tmp/example.json"),
            Err(AuditStoreError::InvalidArtifactPath)
        ));
    }

    #[test]
    fn claims_records_and_finishes_work_atomically() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let plan = store.store_plan(plan()).expect("store plan");
        let events = store.subscribe_events().expect("subscribe events");
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
        assert_eq!(
            recv_event(&events),
            AuditStoreEvent::StageUpdated {
                audit_id: "audit-work".to_string(),
                stage: AuditStageId::BuildNormalize,
                status: AuditStageStatus::Running,
                run: store
                    .read_run("audit-work")
                    .expect("read run")
                    .expect("run exists"),
            }
        );
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
        assert_eq!(
            recv_event(&events),
            AuditStoreEvent::StageUpdated {
                audit_id: "audit-work".to_string(),
                stage: AuditStageId::BuildNormalize,
                status: AuditStageStatus::Succeeded,
                run: update.run,
            }
        );
    }

    #[test]
    fn router_evidence_attaches_to_current_claimed_work() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let plan = store.store_plan(plan()).expect("store plan");
        let stages = vec![AuditStageId::BuildNormalize];
        let run = AuditRun {
            schema_version: 1,
            id: "audit-router".to_string(),
            plan_fingerprint: plan.fingerprint,
            target: plan.target,
            profile: plan.profile,
            status: AuditRunStatus::Running,
            current_stage: stages[0].clone(),
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: Some("adapter/sui".to_string()),
            capabilities: Vec::new(),
            coverage_gaps: Vec::new(),
            work_items: create_audit_work_items("audit-router", &stages, 10),
            evidence_refs: Vec::new(),
            artifact_refs: Vec::new(),
            created_at: 10,
            updated_at: 10,
            metadata: Metadata::new(),
        };
        store.create_run(&run).expect("create run");
        store
            .claim_work("audit-router", "router", None, 11)
            .expect("claim work");
        let evidence = AuditEvidence {
            id: String::new(),
            audit_run_id: String::new(),
            work_item_id: None,
            verification_method: VerificationMethod::StaticAnalysis,
            provider_id: "mcp__sui".to_string(),
            adapter_id: None,
            tool_name: "mcp__sui__static_analysis".to_string(),
            tool_version: None,
            input_hash: "sha256:abc".to_string(),
            source_precision: SourcePrecision::Summary,
            attestation: AuditEvidenceAttestation::RouterCaptured,
            summary: "captured".to_string(),
            observation: "ok".to_string(),
            execution_trace_ref: None,
            artifact_refs: Vec::new(),
            created_at: 12,
            metadata: Metadata::new(),
        };

        let (run, evidence_ref) = store
            .record_router_evidence_for_current_work("audit-router", evidence)
            .expect("record router evidence")
            .expect("claimed work");

        assert_eq!(run.evidence_refs, vec![evidence_ref.clone()]);
        assert_eq!(run.work_items[0].evidence_refs, vec![evidence_ref.clone()]);
        let body =
            std::fs::read_to_string(store.audits_root().join("audit-router").join(evidence_ref))
                .expect("read evidence");
        let stored: AuditEvidence = serde_json::from_str(&body).expect("evidence json");
        assert_eq!(
            stored.work_item_id,
            Some("audit-router:stage:0".to_string())
        );
        assert_eq!(stored.adapter_id, Some("adapter/sui".to_string()));
        assert_eq!(stored.attestation, AuditEvidenceAttestation::RouterCaptured);
    }

    #[test]
    fn finalized_report_accepts_confirmed_finding_with_independent_replay() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let work_item_id = create_report_run(&store, "audit-report-ok");
        store
            .claim_work("audit-report-ok", "judge", None, 11)
            .expect("claim work");
        let (_, static_ref) = store
            .record_evidence(
                "audit-report-ok",
                evidence(
                    &work_item_id,
                    VerificationMethod::StaticAnalysis,
                    AuditEvidenceAttestation::RouterCaptured,
                    None,
                ),
            )
            .expect("record static evidence");
        let (_, replay_ref) = store
            .record_evidence(
                "audit-report-ok",
                evidence(
                    &work_item_id,
                    VerificationMethod::ExploitReplay,
                    AuditEvidenceAttestation::AdapterReplay,
                    Some("traces/replay.json"),
                ),
            )
            .expect("record replay evidence");
        let finding = confirmed_finding(vec![static_ref, replay_ref]);
        let events = store.subscribe_events().expect("subscribe events");

        let finalized = store
            .finalize_report(
                "audit-report-ok",
                vec![finding.clone()],
                Metadata::new(),
                20,
            )
            .expect("finalize report");

        assert_eq!(finalized.run.status, AuditRunStatus::Completed);
        assert_eq!(
            finalized.run.work_items[0].status,
            AuditWorkItemStatus::Completed
        );
        let body = std::fs::read_to_string(
            store
                .audits_root()
                .join("audit-report-ok")
                .join("reports/report.json"),
        )
        .expect("read report");
        let report: AuditReport = serde_json::from_str(&body).expect("report json");
        assert_eq!(report.findings, vec![finding.clone()]);
        assert_eq!(
            recv_event(&events),
            AuditStoreEvent::StageUpdated {
                audit_id: "audit-report-ok".to_string(),
                stage: AuditStageId::AuditTrace,
                status: AuditStageStatus::Succeeded,
                run: finalized.run,
            }
        );
        assert_eq!(
            recv_event(&events),
            AuditStoreEvent::FindingUpdated {
                audit_id: "audit-report-ok".to_string(),
                finding,
                report_ref: "reports/report.json".to_string(),
            }
        );
    }

    #[test]
    fn finalized_report_rejects_confirmed_finding_without_adapter_replay() {
        let home = tempfile::tempdir().expect("tempdir");
        let store = AuditStore::open(home.path()).expect("open store");
        let work_item_id = create_report_run(&store, "audit-report-reject");
        store
            .claim_work("audit-report-reject", "judge", None, 11)
            .expect("claim work");
        let (_, static_ref) = store
            .record_evidence(
                "audit-report-reject",
                evidence(
                    &work_item_id,
                    VerificationMethod::StaticAnalysis,
                    AuditEvidenceAttestation::RouterCaptured,
                    None,
                ),
            )
            .expect("record static evidence");
        let (_, replay_ref) = store
            .record_evidence(
                "audit-report-reject",
                evidence(
                    &work_item_id,
                    VerificationMethod::ExploitReplay,
                    AuditEvidenceAttestation::RouterCaptured,
                    Some("traces/replay.json"),
                ),
            )
            .expect("record replay evidence");

        let error = store
            .finalize_report(
                "audit-report-reject",
                vec![confirmed_finding(vec![static_ref, replay_ref])],
                Metadata::new(),
                20,
            )
            .expect_err("router replay should not confirm finding");

        assert!(matches!(error, AuditStoreError::InvalidReport(_)));
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

    fn create_report_run(store: &AuditStore, audit_id: &str) -> String {
        let plan = store.store_plan(plan()).expect("store plan");
        let stages = vec![AuditStageId::AuditTrace];
        let work_items = create_audit_work_items(audit_id, &stages, 10);
        let work_item_id = work_items[0].id.clone();
        let run = AuditRun {
            schema_version: 1,
            id: audit_id.to_string(),
            plan_fingerprint: plan.fingerprint,
            target: plan.target,
            profile: plan.profile,
            status: AuditRunStatus::Running,
            current_stage: stages[0].clone(),
            coordinator_thread_id: None,
            goal_id: None,
            adapter_id: Some("adapter/sui".to_string()),
            capabilities: Vec::new(),
            coverage_gaps: Vec::new(),
            work_items,
            evidence_refs: Vec::new(),
            artifact_refs: Vec::new(),
            created_at: 10,
            updated_at: 10,
            metadata: Metadata::new(),
        };
        store.create_run(&run).expect("create run");
        work_item_id
    }

    fn evidence(
        work_item_id: &str,
        verification_method: VerificationMethod,
        attestation: AuditEvidenceAttestation,
        execution_trace_ref: Option<&str>,
    ) -> AuditEvidence {
        AuditEvidence {
            id: String::new(),
            audit_run_id: String::new(),
            work_item_id: Some(work_item_id.to_string()),
            verification_method,
            provider_id: "test-provider".to_string(),
            adapter_id: Some("adapter/sui".to_string()),
            tool_name: "test-tool".to_string(),
            tool_version: Some("1".to_string()),
            input_hash: "sha256:abc".to_string(),
            source_precision: SourcePrecision::SourceMap,
            attestation,
            summary: "summary".to_string(),
            observation: "observation".to_string(),
            execution_trace_ref: execution_trace_ref.map(str::to_string),
            artifact_refs: Vec::new(),
            created_at: 12,
            metadata: Metadata::new(),
        }
    }

    fn confirmed_finding(evidence_refs: Vec<String>) -> FindingCandidate {
        FindingCandidate {
            id: "finding-1".to_string(),
            title: "confirmed exploit".to_string(),
            category: "accounting".to_string(),
            severity: FindingCandidateSeverity::High,
            confidence: EvidenceConfidence::Confirmed,
            status: FindingCandidateStatus::Confirmed,
            affected_symbols: vec!["module::entry".to_string()],
            exploit_scenario: Some("attacker drains funds".to_string()),
            evidence_refs,
            validation_plan: ValidationPlan {
                commands: Vec::new(),
                expected_evidence: Vec::new(),
                required: true,
            },
            patch_recommendation: None,
            metadata: Metadata::new(),
        }
    }

    fn recv_event(receiver: &std::sync::mpsc::Receiver<AuditStoreEvent>) -> AuditStoreEvent {
        receiver
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("audit event")
    }
}
