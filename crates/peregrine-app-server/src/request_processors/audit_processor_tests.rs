use super::*;
use crate::outgoing_message::OutgoingEnvelope;
use crate::thread_state::ThreadStateManager;
use codex_analytics::AnalyticsEventsClient;
use codex_extension_api::empty_extension_registry;
use codex_login::CodexAuth;
use core_test_support::load_default_config_for_test;
use peregrine_core::init_state_db;
use peregrine_core::thread_store_from_config;
use peregrine_security_tools::{
    AcquiredAuditTarget, AdapterFuture, AuditChainAdapter, AuditTargetPreflight, ExploitReplay,
    default_audit_stages,
};
use peregrine_types::protocol::SessionSource;
use peregrine_types::{
    AuditAgentAssignmentStatus, AuditAgentRole, AuditCapabilityBinding, AuditPlannerOutput,
    AuditProfile, AuditStagePlan, AuditTarget, ExploitBundle, ExploitIntent,
};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use tempfile::{TempDir, tempdir};
use tokio::sync::mpsc;

struct RecordingContinuation {
    calls: AtomicUsize,
}

impl RecordingContinuation {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
        })
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl AuditCoordinatorContinuation for RecordingContinuation {
    fn continue_if_idle<'a>(&'a self, _thread: &'a PeregrineThread) -> AuditContinuationFuture<'a> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Ok(()) })
    }
}

struct TestAuditAdapter;

impl AuditChainAdapter for TestAuditAdapter {
    fn adapter_id(&self) -> &'static str {
        "test-adapter"
    }

    fn chain_id(&self) -> &'static str {
        "test-chain"
    }

    fn capabilities(&self) -> Vec<AuditCapabilityBinding> {
        vec![
            AuditCapabilityBinding {
                capability: "target.acquire".to_string(),
                provider_id: "test-adapter".to_string(),
                adapter_id: Some(self.adapter_id().to_string()),
                tool_name: None,
                available: true,
                diagnostic: None,
            },
            AuditCapabilityBinding {
                capability: "target.normalize".to_string(),
                provider_id: "test-adapter".to_string(),
                adapter_id: Some(self.adapter_id().to_string()),
                tool_name: None,
                available: true,
                diagnostic: None,
            },
        ]
    }

    fn preflight<'a>(&'a self, target: &'a AuditTarget) -> AdapterFuture<'a, AuditTargetPreflight> {
        Box::pin(async move {
            Ok(AuditTargetPreflight {
                adapter_id: self.adapter_id().to_string(),
                normalized_target: target.clone(),
                capabilities: self.capabilities(),
                diagnostics: Vec::new(),
            })
        })
    }

    fn acquire<'a>(
        &'a self,
        _target: &'a AuditTarget,
        _profile: &'a AuditProfile,
        workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, AcquiredAuditTarget> {
        Box::pin(async move {
            Ok(AcquiredAuditTarget {
                adapter_id: self.adapter_id().to_string(),
                root: workspace.input.clone(),
                manifest_ref: "artifacts/manifest.json".to_string(),
                artifact_refs: Vec::new(),
                immutable_state_ref: None,
                diagnostics: Vec::new(),
                metadata: Metadata::new(),
            })
        })
    }

    fn encode_exploit<'a>(
        &'a self,
        _target: &'a AcquiredAuditTarget,
        intent: &'a ExploitIntent,
        _workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitBundle> {
        Box::pin(async move {
            Ok(ExploitBundle {
                id: "bundle-1".to_string(),
                adapter_id: self.adapter_id().to_string(),
                intent_id: intent.id.clone(),
                format: "test".to_string(),
                artifact_refs: Vec::new(),
                replayable: false,
                metadata: Metadata::new(),
            })
        })
    }

    fn replay_exploit<'a>(
        &'a self,
        _target: &'a AcquiredAuditTarget,
        bundle: &'a ExploitBundle,
        _workspace: &'a AuditWorkspace,
    ) -> AdapterFuture<'a, ExploitReplay> {
        Box::pin(async move {
            Ok(ExploitReplay {
                bundle_id: bundle.id.clone(),
                succeeded: false,
                evidence_refs: Vec::new(),
                diagnostics: Vec::new(),
                metadata: Metadata::new(),
            })
        })
    }
}

struct ProcessorFixture {
    processor: AuditRequestProcessor,
    thread_manager: Arc<ThreadManager>,
    continuation: Arc<RecordingContinuation>,
    plan_fingerprint: String,
    _peregrine_home: TempDir,
    _target_root: TempDir,
}

async fn processor_fixture() -> anyhow::Result<ProcessorFixture> {
    let peregrine_home = tempdir()?;
    let target_root = tempdir()?;
    let mut config = load_default_config_for_test(&peregrine_home).await;
    config
        .features
        .enable(Feature::Goals)
        .expect("goals should be enableable in tests");
    let config = Arc::new(config);
    let auth_manager = AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy"));
    let state_db = init_state_db(config.as_ref())
        .await
        .expect("audit processor tests require state db");
    let thread_store = thread_store_from_config(config.as_ref(), Some(state_db.clone()));
    let thread_manager = Arc::new(ThreadManager::new(
        config.as_ref(),
        auth_manager.clone(),
        SessionSource::Exec,
        Arc::new(codex_exec_server::EnvironmentManager::default_for_tests()),
        empty_extension_registry(),
        /*analytics_events_client*/ None,
        thread_store,
        Some(state_db.clone()),
        "11111111-1111-4111-8111-111111111111".to_string(),
        /*attestation_provider*/ None,
    ));
    let (outgoing_tx, _outgoing_rx) = mpsc::channel::<OutgoingEnvelope>(64);
    let outgoing = Arc::new(OutgoingMessageSender::new(
        outgoing_tx,
        AnalyticsEventsClient::disabled(),
    ));
    let thread_goal_processor = ThreadGoalRequestProcessor::new(
        thread_manager.clone(),
        outgoing.clone(),
        config.clone(),
        ThreadStateManager::new(),
        Some(state_db.clone()),
    );
    let mut adapters = AuditAdapterRegistry::default();
    adapters.register(Arc::new(TestAuditAdapter));
    let continuation = RecordingContinuation::new();
    let processor = AuditRequestProcessor::new(
        auth_manager,
        thread_manager.clone(),
        thread_goal_processor,
        outgoing,
        config.clone(),
        Some(state_db),
        Arc::new(adapters),
    )
    .with_coordinator_continuation_for_tests(continuation.clone());
    let plan = AuditPlan {
        schema_version: 1,
        id: "plan-1".to_string(),
        fingerprint: String::new(),
        target: AuditTarget::LocalPackage {
            chain_id: "test-chain".to_string(),
            path: target_root.path().display().to_string(),
            metadata: Metadata::new(),
        },
        profile: AuditProfile::default(),
        stages: default_audit_stages(),
        desired_capabilities: vec![
            "target.acquire".to_string(),
            "target.normalize".to_string(),
            "static.analysis".to_string(),
            "graph.analysis".to_string(),
            "bytecode.analysis".to_string(),
            "dynamic.fuzzing".to_string(),
            "symbolic.execution".to_string(),
            "economic.simulation".to_string(),
            "exploit.replay".to_string(),
        ],
        planner_output: test_planner_output(),
        created_at: 1,
        metadata: Metadata::new(),
    };
    let plan = AuditStore::open(&config.peregrine_home)?.store_plan(plan)?;
    Ok(ProcessorFixture {
        processor,
        thread_manager,
        continuation,
        plan_fingerprint: plan.fingerprint,
        _peregrine_home: peregrine_home,
        _target_root: target_root,
    })
}

fn test_planner_output() -> AuditPlannerOutput {
    AuditPlannerOutput {
        summary: "Test model-authored audit plan".to_string(),
        rationale: "Exercise the persisted audit lifecycle with a typed planner output."
            .to_string(),
        focus_areas: vec!["entry points".to_string()],
        non_goals: Vec::new(),
        stage_plans: vec![
            AuditStagePlan {
                stage: AuditStageId::BuildNormalize,
                objective: "Normalize the test target.".to_string(),
                rationale: "Every audit needs a normalized target.".to_string(),
                focus: vec!["manifest".to_string()],
                desired_capabilities: vec!["target.normalize".to_string()],
                agent_roles: Vec::new(),
                success_criteria: vec!["target acquired".to_string()],
            },
            AuditStagePlan {
                stage: AuditStageId::AuditReport,
                objective: "Write the test report.".to_string(),
                rationale: "Every audit must terminate in a report.".to_string(),
                focus: vec!["summary".to_string()],
                desired_capabilities: Vec::new(),
                agent_roles: vec![AuditAgentRole::Judge],
                success_criteria: vec!["report persisted".to_string()],
            },
        ],
        acceptance_criteria: vec!["audit report exists".to_string()],
    }
}

#[tokio::test]
async fn audit_start_and_resume_continue_coordinator_goal() -> anyhow::Result<()> {
    let fixture = processor_fixture().await?;
    let start = fixture
        .processor
        .start(AuditStartParams {
            fingerprint: fixture.plan_fingerprint,
        })
        .await
        .map_err(|error| anyhow::anyhow!("audit start failed: {error:?}"))?;
    let run: AuditRun = serde_json::from_value(start.run)?;

    assert_eq!(run.status, AuditRunStatus::Running);
    assert!(run.goal_id.is_some());
    assert!(run.coordinator_thread_id.is_some());
    let first_schedule = run.work_items[0]
        .metadata
        .get("stageSchedule")
        .expect("stage schedule")
        .as_object()
        .expect("stage schedule object");
    assert_eq!(
        first_schedule.get("action"),
        Some(&serde_json::json!("useAvailableCapabilities"))
    );
    assert_eq!(
        first_schedule.get("desiredCapabilities"),
        Some(&serde_json::json!(["target.acquire", "target.normalize"]))
    );
    assert_eq!(run.agent_assignments.len(), 13);
    assert!(run.agent_assignments.iter().all(|assignment| {
        assignment.audit_run_id == run.id
            && assignment.status == AuditAgentAssignmentStatus::Pending
            && assignment.agent_thread_id.is_none()
            && assignment.conclusion_refs.is_empty()
    }));
    assert!(run.agent_assignments.iter().any(|assignment| {
        assignment.role == AuditAgentRole::Researcher && assignment.role_name == "audit-researcher"
    }));
    assert!(run.agent_assignments.iter().any(|assignment| {
        assignment.role == AuditAgentRole::Skeptic && assignment.role_name == "audit-skeptic"
    }));
    assert!(run.agent_assignments.iter().any(|assignment| {
        assignment.role == AuditAgentRole::Exploiter && assignment.role_name == "audit-exploiter"
    }));
    assert!(run.agent_assignments.iter().any(|assignment| {
        assignment.role == AuditAgentRole::Judge && assignment.role_name == "audit-judge"
    }));
    let researcher_assignment = run
        .agent_assignments
        .iter()
        .find(|assignment| assignment.role == AuditAgentRole::Researcher)
        .expect("researcher assignment");
    assert_eq!(
        researcher_assignment.metadata.get("roleSource"),
        Some(&serde_json::json!("builtIn"))
    );
    assert_eq!(
        researcher_assignment.metadata.get("roleConfigFile"),
        Some(&serde_json::json!("audit_researcher.toml"))
    );
    let bindings = run
        .metadata
        .get("agentRoleBindings")
        .and_then(serde_json::Value::as_array)
        .expect("agent role bindings");
    assert_eq!(bindings.len(), run.agent_assignments.len());
    assert!(bindings.iter().any(|binding| {
        binding.get("roleName") == Some(&serde_json::json!("audit-judge"))
            && binding.get("source") == Some(&serde_json::json!("builtIn"))
    }));
    assert_eq!(fixture.continuation.calls(), 1);

    fixture
        .processor
        .pause(AuditLifecycleParams {
            audit_id: run.id.clone(),
        })
        .await
        .map_err(|error| anyhow::anyhow!("audit pause failed: {error:?}"))?;
    assert_eq!(fixture.continuation.calls(), 1);

    fixture
        .processor
        .resume(AuditLifecycleParams { audit_id: run.id })
        .await
        .map_err(|error| anyhow::anyhow!("audit resume failed: {error:?}"))?;
    assert_eq!(fixture.continuation.calls(), 2);

    fixture
        .thread_manager
        .shutdown_all_threads_bounded(Duration::from_secs(5))
        .await;
    Ok(())
}
