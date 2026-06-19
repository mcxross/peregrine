use peregrine_types::{
    AuditCapabilityBinding, AuditStageId, AuditWorkItem, Metadata, VerificationMethod,
};
use serde::{Deserialize, Serialize};

pub const STAGE_SCHEDULE_METADATA_KEY: &str = "stageSchedule";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditStageSchedule {
    pub schema_version: u8,
    pub stage: AuditStageId,
    pub action: AuditStageScheduleAction,
    pub required_capabilities: Vec<String>,
    pub available_capabilities: Vec<AuditStageAvailableCapability>,
    pub unavailable_capabilities: Vec<AuditStageUnavailableCapability>,
    pub verification_methods: Vec<VerificationMethod>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum AuditStageScheduleAction {
    ModelOnly,
    UseAvailableCapabilities,
    RecordUnavailableAndContinue,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditStageAvailableCapability {
    pub capability: String,
    pub provider_id: String,
    pub adapter_id: Option<String>,
    pub tool_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuditStageUnavailableCapability {
    pub capability: String,
    pub reason: String,
}

pub fn attach_stage_schedules(
    work_items: &mut [AuditWorkItem],
    capabilities: &[AuditCapabilityBinding],
) -> Result<(), serde_json::Error> {
    for work_item in work_items {
        let schedule = stage_schedule(&work_item.stage, capabilities);
        work_item.metadata.insert(
            STAGE_SCHEDULE_METADATA_KEY.to_string(),
            serde_json::to_value(schedule)?,
        );
    }
    Ok(())
}

pub fn stage_schedule(
    stage: &AuditStageId,
    capabilities: &[AuditCapabilityBinding],
) -> AuditStageSchedule {
    let required_capabilities = stage_required_capabilities(stage)
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    let mut available_capabilities = Vec::new();
    let mut unavailable_capabilities = Vec::new();
    for capability in &required_capabilities {
        match capabilities
            .iter()
            .find(|binding| binding.capability == *capability)
        {
            Some(binding) if binding.available => {
                available_capabilities.push(AuditStageAvailableCapability {
                    capability: capability.clone(),
                    provider_id: binding.provider_id.clone(),
                    adapter_id: binding.adapter_id.clone(),
                    tool_name: binding.tool_name.clone(),
                });
            }
            Some(binding) => {
                unavailable_capabilities.push(AuditStageUnavailableCapability {
                    capability: capability.clone(),
                    reason: binding
                        .diagnostic
                        .clone()
                        .unwrap_or_else(|| "capability provider is unavailable".to_string()),
                });
            }
            None => {
                unavailable_capabilities.push(AuditStageUnavailableCapability {
                    capability: capability.clone(),
                    reason: "no capability provider is registered".to_string(),
                });
            }
        }
    }
    let action = if required_capabilities.is_empty() {
        AuditStageScheduleAction::ModelOnly
    } else if unavailable_capabilities.is_empty() {
        AuditStageScheduleAction::UseAvailableCapabilities
    } else {
        AuditStageScheduleAction::RecordUnavailableAndContinue
    };
    AuditStageSchedule {
        schema_version: 1,
        stage: stage.clone(),
        action,
        verification_methods: verification_methods_for_stage(stage),
        required_capabilities,
        available_capabilities,
        unavailable_capabilities,
    }
}

pub fn stage_required_capabilities(stage: &AuditStageId) -> Vec<&'static str> {
    match stage {
        AuditStageId::BuildNormalize => vec!["target.acquire", "target.normalize"],
        AuditStageId::SemanticGraphs
        | AuditStageId::GraphAnalysis
        | AuditStageId::AttackSurface
        | AuditStageId::AttackHypotheses => vec!["static.analysis", "graph.analysis"],
        AuditStageId::StaticAnalysis
        | AuditStageId::FunctionRiskMap
        | AuditStageId::Invariants
        | AuditStageId::ThreatModel
        | AuditStageId::Classification => vec!["static.analysis"],
        AuditStageId::BytecodeReview => vec!["bytecode.analysis"],
        AuditStageId::DynamicAnalysis | AuditStageId::InvariantStress => vec!["dynamic.fuzzing"],
        AuditStageId::SymbolicExecution => vec!["symbolic.execution"],
        AuditStageId::EconomicSimulation => vec!["economic.simulation"],
        AuditStageId::TargetedTests | AuditStageId::ExploitConfirmation => vec!["exploit.replay"],
        AuditStageId::FindingValidation => vec!["exploit.replay"],
        AuditStageId::AuditSession
        | AuditStageId::VerificationPlanning
        | AuditStageId::AdversarialReview
        | AuditStageId::EvidenceAggregation
        | AuditStageId::SeverityRanking
        | AuditStageId::Remediation
        | AuditStageId::RegressionTests
        | AuditStageId::AuditReport
        | AuditStageId::AuditTrace
        | AuditStageId::FixVerification => Vec::new(),
    }
}

fn verification_methods_for_stage(stage: &AuditStageId) -> Vec<VerificationMethod> {
    match stage {
        AuditStageId::SemanticGraphs | AuditStageId::GraphAnalysis => {
            vec![VerificationMethod::GraphAnalysis]
        }
        AuditStageId::AttackSurface
        | AuditStageId::AttackHypotheses
        | AuditStageId::StaticAnalysis
        | AuditStageId::FunctionRiskMap
        | AuditStageId::Invariants
        | AuditStageId::ThreatModel
        | AuditStageId::Classification => vec![VerificationMethod::StaticAnalysis],
        AuditStageId::BytecodeReview => vec![VerificationMethod::BytecodeAnalysis],
        AuditStageId::DynamicAnalysis | AuditStageId::InvariantStress => {
            vec![VerificationMethod::Fuzzing]
        }
        AuditStageId::SymbolicExecution => vec![VerificationMethod::SymbolicExecution],
        AuditStageId::EconomicSimulation => vec![VerificationMethod::EconomicSimulation],
        AuditStageId::TargetedTests | AuditStageId::ExploitConfirmation => {
            vec![VerificationMethod::ExploitReplay]
        }
        AuditStageId::FindingValidation => {
            vec![
                VerificationMethod::ExploitReplay,
                VerificationMethod::HumanReview,
            ]
        }
        AuditStageId::BuildNormalize
        | AuditStageId::AuditSession
        | AuditStageId::VerificationPlanning
        | AuditStageId::AdversarialReview
        | AuditStageId::EvidenceAggregation
        | AuditStageId::SeverityRanking
        | AuditStageId::Remediation
        | AuditStageId::RegressionTests
        | AuditStageId::AuditReport
        | AuditStageId::AuditTrace
        | AuditStageId::FixVerification => Vec::new(),
    }
}

pub fn schedule_metadata(metadata: &Metadata) -> Option<AuditStageSchedule> {
    metadata
        .get(STAGE_SCHEDULE_METADATA_KEY)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
}
