use peregrine_move_model::MoveModule;
use peregrine_scanner::sui::objects::{
    ObjectLifecycleFunctionRef as ScannerLifecycleFunctionRef,
    ObjectLifecycleModel as ScannerLifecycleModel,
    ObjectLifecycleStageModel as ScannerLifecycleStageModel, ObjectScanReport,
};
use std::collections::HashSet;

use crate::object_lifecycle::{
    ObjectLifecycleFunctionRef, ObjectLifecycleMap, ObjectLifecycleStage, object_lifecycle_risks,
};

use super::types::{CapabilityFinding, ObjectOwnershipFinding};

pub(super) fn capability_findings_from_object_scan(
    report: &ObjectScanReport,
) -> Vec<CapabilityFinding> {
    report
        .capability_findings
        .iter()
        .map(|finding| CapabilityFinding {
            type_name: finding.type_name.clone(),
            module_name: finding.module_name.clone(),
            qualified_name: finding.qualified_name.clone(),
            confidence: finding.confidence.as_str().to_string(),
            evidence: finding
                .evidence
                .iter()
                .map(|evidence| evidence.message.clone())
                .collect(),
            protected_functions: finding.protected_functions.clone(),
        })
        .collect()
}

pub(super) fn object_ownership_findings_from_object_scan(
    report: &ObjectScanReport,
) -> Vec<ObjectOwnershipFinding> {
    report
        .ownership_findings
        .iter()
        .map(|finding| ObjectOwnershipFinding {
            type_name: finding.type_name.clone(),
            module_name: finding.module_name.clone(),
            qualified_name: finding.qualified_name.clone(),
            ownership_kind: finding.ownership_kind.as_str().to_string(),
            confidence: finding.confidence.as_str().to_string(),
            evidence: finding
                .evidence
                .iter()
                .map(|evidence| evidence.message.clone())
                .collect(),
            related_functions: finding.related_functions.clone(),
            wrapped_types: finding.wrapped_types.clone(),
        })
        .collect()
}

pub(super) fn object_lifecycle_maps_from_object_scan(
    report: &ObjectScanReport,
    modules: &[MoveModule],
) -> Vec<ObjectLifecycleMap> {
    report
        .lifecycle_maps
        .iter()
        .map(|lifecycle| {
            let mut map = lifecycle_map_from_scanner(lifecycle);
            map.risks = object_lifecycle_risks(modules, &map);
            map
        })
        .collect()
}

fn lifecycle_map_from_scanner(lifecycle: &ScannerLifecycleModel) -> ObjectLifecycleMap {
    ObjectLifecycleMap {
        type_name: lifecycle.type_name.clone(),
        module_name: lifecycle.module_name.clone(),
        qualified_name: lifecycle.qualified_name.clone(),
        file_path: lifecycle.file_path.clone(),
        abilities: lifecycle.abilities.clone(),
        is_capability_like: lifecycle.is_capability_like,
        stages: lifecycle
            .stages
            .iter()
            .map(lifecycle_stage_from_scanner)
            .collect(),
        touched_by: lifecycle
            .touched_by
            .iter()
            .map(lifecycle_function_ref_from_scanner)
            .collect(),
        risks: Vec::new(),
    }
}

fn lifecycle_stage_from_scanner(stage: &ScannerLifecycleStageModel) -> ObjectLifecycleStage {
    ObjectLifecycleStage {
        kind: stage.kind.as_str().to_string(),
        functions: stage
            .functions
            .iter()
            .map(lifecycle_function_ref_from_scanner)
            .collect(),
        evidence: stage
            .evidence
            .iter()
            .map(|evidence| evidence.message.clone())
            .collect(),
    }
}

fn lifecycle_function_ref_from_scanner(
    function: &ScannerLifecycleFunctionRef,
) -> ObjectLifecycleFunctionRef {
    ObjectLifecycleFunctionRef {
        module_name: function.module_name.clone(),
        function_name: function.function_name.clone(),
        qualified_name: function.qualified_name.clone(),
        file_path: function.file_path.clone(),
        visibility: function.visibility.clone(),
        is_entry: function.is_entry,
        is_transaction_callable: function.is_transaction_callable,
        direct: function.direct,
        call_path: function.call_path.clone(),
        evidence: function
            .evidence
            .iter()
            .map(|evidence| evidence.message.clone())
            .collect(),
    }
}

pub(super) fn ownership_count(findings: &[ObjectOwnershipFinding], kind: &str) -> usize {
    findings
        .iter()
        .filter(|finding| finding.ownership_kind == kind && finding.confidence != "low")
        .map(|finding| finding.qualified_name.as_str())
        .collect::<HashSet<_>>()
        .len()
}
