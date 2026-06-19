use crate::error_code::internal_error;
use crate::error_code::invalid_request;
use peregrine_app_server_protocol::{AuditProfileParams, AuditTargetParams, JSONRPCErrorError};
use peregrine_types::{
    AuditCapabilityBinding, AuditCoverageGap, AuditPlan, AuditProfile, AuditRun, AuditStageId,
    AuditTarget, ThreadId,
};

pub(super) fn profile_from_params(value: AuditProfileParams) -> AuditProfile {
    AuditProfile {
        model_token_budget: value.model_token_budget,
        wall_time_seconds: value.wall_time_seconds,
        max_hypotheses: value.max_hypotheses,
        max_dependency_depth: value.max_dependency_depth,
        max_dependency_packages: value.max_dependency_packages,
    }
}

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

pub(super) fn target_from_params(value: AuditTargetParams) -> AuditTarget {
    match value {
        AuditTargetParams::LocalPackage {
            chain_id,
            path,
            metadata,
        } => AuditTarget::LocalPackage {
            chain_id,
            path,
            metadata: metadata.unwrap_or_default().into_iter().collect(),
        },
        AuditTargetParams::RemotePackage {
            chain_id,
            network_id,
            package_ref,
            source_uri,
            state_ref,
            metadata,
        } => AuditTarget::RemotePackage {
            chain_id,
            network_id,
            package_ref,
            source_uri,
            state_ref,
            metadata: metadata.unwrap_or_default().into_iter().collect(),
        },
    }
}

pub(super) fn default_required_capabilities() -> Vec<String> {
    [
        "target.acquire",
        "target.normalize",
        "static.analysis",
        "graph.analysis",
        "dynamic.fuzzing",
        "symbolic.execution",
        "economic.simulation",
        "exploit.replay",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

pub(super) fn coverage_gaps(
    plan: &AuditPlan,
    capabilities: &[AuditCapabilityBinding],
) -> Vec<AuditCoverageGap> {
    plan.required_capabilities
        .iter()
        .filter(|required| {
            !capabilities
                .iter()
                .any(|binding| binding.capability == **required && binding.available)
        })
        .map(|capability| AuditCoverageGap {
            capability: capability.clone(),
            stage: capability_stage(capability),
            reason: capabilities
                .iter()
                .find(|binding| binding.capability == *capability)
                .and_then(|binding| binding.diagnostic.clone())
                .unwrap_or_else(|| "no capability provider is registered".to_string()),
            required: true,
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
