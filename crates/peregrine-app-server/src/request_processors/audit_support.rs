use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use peregrine_app_server_protocol::JSONRPCErrorError;
use peregrine_types::{
    AuditCapabilityBinding, AuditCoverageGap, AuditPlan, AuditProfile, AuditRun, AuditStageId,
    ThreadId,
};

pub(super) fn validate_profile(profile: &AuditProfile) -> Result<(), JSONRPCErrorError> {
    if profile.model_token_budget <= 0 {
        return Err(invalid_request(
            "modelTokenBudget must be greater than zero",
        ));
    }
    if profile.wall_time_seconds <= 0 {
        return Err(invalid_request("wallTimeSeconds must be greater than zero"));
    }
    if profile.max_hypotheses == 0 {
        return Err(invalid_request("maxHypotheses must be greater than zero"));
    }
    if !(1..=16).contains(&profile.max_dependency_depth) {
        return Err(invalid_request(
            "maxDependencyDepth must be between 1 and 16",
        ));
    }
    if !(1..=512).contains(&profile.max_dependency_packages) {
        return Err(invalid_request(
            "maxDependencyPackages must be between 1 and 512",
        ));
    }
    Ok(())
}

pub(super) fn coverage_gaps(
    plan: &AuditPlan,
    capabilities: &[AuditCapabilityBinding],
) -> Vec<AuditCoverageGap> {
    plan.desired_capabilities
        .iter()
        .filter_map(|capability| {
            let binding = capabilities
                .iter()
                .find(|binding| binding.capability == *capability)?;
            (!binding.available).then_some((capability, binding))
        })
        .map(|(capability, binding)| AuditCoverageGap {
            capability: capability.clone(),
            stage: capability_stage(capability),
            reason: binding
                .diagnostic
                .clone()
                .unwrap_or_else(|| "capability provider is unavailable".to_string()),
            affects_terminal_status: true,
        })
        .collect()
}

fn capability_stage(capability: &str) -> AuditStageId {
    match capability {
        "symbolic.execution" => AuditStageId::SymbolicExecution,
        "economic.simulation" => AuditStageId::EconomicSimulation,
        "exploit.replay" => AuditStageId::ExploitConfirmation,
        "dynamic.fuzzing" => AuditStageId::DynamicAnalysis,
        "graph.analysis" => AuditStageId::GraphAnalysis,
        "static.analysis" => AuditStageId::StaticAnalysis,
        "bytecode.analysis" => AuditStageId::BytecodeReview,
        "formal.verification" => AuditStageId::InvariantStress,
        _ => AuditStageId::BuildNormalize,
    }
}

pub(super) fn parse_coordinator_thread_id(run: &AuditRun) -> Result<ThreadId, JSONRPCErrorError> {
    ThreadId::from_string(
        run.coordinator_thread_id
            .as_deref()
            .ok_or_else(|| invalid_request("audit coordinator thread is missing"))?,
    )
    .map_err(|error| invalid_request(format!("invalid coordinator thread id: {error}")))
}

pub(super) fn serialize(
    value: &impl serde::Serialize,
) -> Result<serde_json::Value, JSONRPCErrorError> {
    serde_json::to_value(value)
        .map_err(|error| internal_error(format!("failed to serialize audit value: {error}")))
}
