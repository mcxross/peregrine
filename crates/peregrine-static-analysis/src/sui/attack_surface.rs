use peregrine_scanner::{
    core::{ScanInput, ScannerOutput, SourceMode},
    sui::{
        objects::{
            ObjectLifecycleFunctionRef as ScannerLifecycleFunctionRef,
            ObjectLifecycleModel as ScannerLifecycleModel,
            ObjectLifecycleStageModel as ScannerLifecycleStageModel, ObjectScanReport,
        },
        scan_package,
    },
};
use peregrine_types::sui::move_model::{MoveFunctionSignature, MoveModule, MovePackageModel};
use serde::Serialize;
use std::collections::HashSet;
use std::path::PathBuf;

use super::object_lifecycle::{
    object_lifecycle_risks, ObjectLifecycleFunctionRef, ObjectLifecycleMap, ObjectLifecycleStage,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackageSurface {
    pub entry_function_count: usize,
    pub capability_count: usize,
    pub shared_object_count: usize,
    pub address_owned_object_count: usize,
    pub immutable_object_count: usize,
    pub wrapped_object_count: usize,
    pub party_object_count: usize,
    pub admin_control_count: usize,
    pub external_call_count: usize,
    pub public_package_relationship_count: usize,
    pub capability_structs: Vec<String>,
    pub capability_findings: Vec<CapabilityFinding>,
    pub shared_object_structs: Vec<String>,
    pub object_lifecycle_maps: Vec<ObjectLifecycleMap>,
    pub object_ownership_findings: Vec<ObjectOwnershipFinding>,
    pub admin_control_findings: Vec<AdminControlFinding>,
    pub external_call_findings: Vec<ExternalCallFinding>,
    pub public_package_relationships: Vec<PublicPackageRelationship>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityFinding {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub protected_functions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectOwnershipFinding {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub ownership_kind: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub related_functions: Vec<String>,
    pub wrapped_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AdminControlFinding {
    pub function_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub confidence: String,
    pub evidence: Vec<String>,
    pub guarding_types: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExternalCallFinding {
    pub caller_module: String,
    pub caller_function: String,
    pub target_module: String,
    pub target_function: String,
    pub target: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicPackageRelationship {
    pub source_module: String,
    pub source_function: String,
    pub target_module: String,
    pub target_function: String,
}

pub fn package_surface(modules: &[MoveModule]) -> MovePackageSurface {
    let model = MovePackageModel {
        name: "package".to_string(),
        path: String::new(),
        manifest_path: String::new(),
        modules: modules.to_vec(),
    };

    package_surface_for_package(&model, None, None)
}

pub fn package_surface_for_package(
    model: &MovePackageModel,
    package_root: Option<PathBuf>,
    build_root: Option<PathBuf>,
) -> MovePackageSurface {
    let modules = &model.modules;
    let entry_function_count = modules
        .iter()
        .flat_map(|module| module.functions.iter())
        .filter(|function| function.is_transaction_callable)
        .count();
    let object_scan = scanner_object_report(model, package_root, build_root);
    let capability_findings = scanner_capability_findings(&object_scan);
    let mut capability_structs = capability_findings
        .iter()
        .filter(|finding| finding.confidence != "low")
        .map(|finding| finding.qualified_name.clone())
        .collect::<Vec<_>>();
    let object_ownership_findings = scanner_object_ownership_findings(&object_scan);
    let object_lifecycle_maps = scanner_object_lifecycle_maps(&object_scan, modules);
    let admin_control_findings = admin_control_findings(modules, &capability_structs);
    let external_call_findings = external_call_findings(modules);
    let public_package_relationships = public_package_relationships(modules);
    let mut shared_object_structs = object_scan.shared_object_structs.clone();

    capability_structs.sort();
    capability_structs.dedup();
    shared_object_structs.sort();
    shared_object_structs.dedup();

    MovePackageSurface {
        entry_function_count,
        capability_count: capability_structs.len(),
        shared_object_count: ownership_count(&object_ownership_findings, "shared")
            .max(shared_object_structs.len()),
        address_owned_object_count: ownership_count(&object_ownership_findings, "addressOwned"),
        immutable_object_count: ownership_count(&object_ownership_findings, "immutable"),
        wrapped_object_count: ownership_count(&object_ownership_findings, "wrapped"),
        party_object_count: ownership_count(&object_ownership_findings, "party"),
        admin_control_count: admin_control_findings.len(),
        external_call_count: external_call_findings.len(),
        public_package_relationship_count: public_package_relationships.len(),
        capability_structs,
        capability_findings,
        shared_object_structs,
        object_lifecycle_maps,
        object_ownership_findings,
        admin_control_findings,
        external_call_findings,
        public_package_relationships,
    }
}

fn scanner_object_report(
    model: &MovePackageModel,
    package_root: Option<PathBuf>,
    build_root: Option<PathBuf>,
) -> ObjectScanReport {
    let report = scan_package(ScanInput {
        package_model: model,
        package_root,
        build_root,
        source_mode: SourceMode::BestAvailable,
    });

    report
        .scanners
        .into_iter()
        .find_map(|output| match output {
            ScannerOutput::Objects(objects) => Some(objects),
        })
        .unwrap_or_else(|| ObjectScanReport {
            capability_findings: Vec::new(),
            ownership_findings: Vec::new(),
            lifecycle_maps: Vec::new(),
            shared_object_structs: Vec::new(),
            diagnostics: report.diagnostics,
        })
}

fn scanner_capability_findings(report: &ObjectScanReport) -> Vec<CapabilityFinding> {
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

fn scanner_object_ownership_findings(report: &ObjectScanReport) -> Vec<ObjectOwnershipFinding> {
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

fn scanner_object_lifecycle_maps(
    report: &ObjectScanReport,
    modules: &[MoveModule],
) -> Vec<ObjectLifecycleMap> {
    report
        .lifecycle_maps
        .iter()
        .map(|lifecycle| {
            let mut map = scanner_lifecycle_map(lifecycle);
            map.risks = object_lifecycle_risks(modules, &map);
            map
        })
        .collect()
}

fn scanner_lifecycle_map(lifecycle: &ScannerLifecycleModel) -> ObjectLifecycleMap {
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
            .map(scanner_lifecycle_stage)
            .collect(),
        touched_by: lifecycle
            .touched_by
            .iter()
            .map(scanner_lifecycle_function_ref)
            .collect(),
        risks: Vec::new(),
    }
}

fn scanner_lifecycle_stage(stage: &ScannerLifecycleStageModel) -> ObjectLifecycleStage {
    ObjectLifecycleStage {
        kind: stage.kind.as_str().to_string(),
        functions: stage
            .functions
            .iter()
            .map(scanner_lifecycle_function_ref)
            .collect(),
        evidence: stage
            .evidence
            .iter()
            .map(|evidence| evidence.message.clone())
            .collect(),
    }
}

fn scanner_lifecycle_function_ref(
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

fn ownership_count(findings: &[ObjectOwnershipFinding], kind: &str) -> usize {
    findings
        .iter()
        .filter(|finding| finding.ownership_kind == kind && finding.confidence != "low")
        .map(|finding| finding.qualified_name.as_str())
        .collect::<HashSet<_>>()
        .len()
}

fn function_parameters_contain_type(signature: &str, type_name: &str) -> bool {
    let Some(parameters) = function_parameters(signature) else {
        return false;
    };

    type_reference_matches(parameters, type_name)
}

fn function_parameters(signature: &str) -> Option<&str> {
    let start = signature.find('(')?;
    let mut depth = 0_i32;

    for (offset, character) in signature[start..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;

                if depth == 0 {
                    return Some(&signature[start + 1..start + offset]);
                }
            }
            _ => {}
        }
    }

    None
}

fn type_reference_matches(source: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    source
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '&' | ',' | ':' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ';'
                )
        })
        .any(|token| token == short_name || token == type_name)
}

fn privileged_function(function: &MoveFunctionSignature) -> bool {
    let name = function.name.to_ascii_lowercase();
    let body = function.body.as_deref().unwrap_or("").to_ascii_lowercase();
    const PRIVILEGED_TERMS: &[&str] = &[
        "admin", "burn", "claim", "config", "create", "destroy", "fee", "mint", "owner", "pause",
        "set", "transfer", "treasury", "unpause", "update", "upgrade", "withdraw",
    ];
    const PRIVILEGED_BODY_TERMS: &[&str] = &[
        "balance::",
        "coin::",
        "dynamic_field",
        "event::emit",
        "object::new",
        "share_object",
        "transfer::",
        "tx_context::sender",
    ];

    PRIVILEGED_TERMS.iter().any(|term| name.contains(term))
        || PRIVILEGED_BODY_TERMS.iter().any(|term| body.contains(term))
}

fn admin_control_findings(
    modules: &[MoveModule],
    capability_structs: &[String],
) -> Vec<AdminControlFinding> {
    let capability_names = capability_structs
        .iter()
        .map(|qualified| {
            qualified
                .rsplit("::")
                .next()
                .unwrap_or(qualified)
                .to_string()
        })
        .collect::<HashSet<_>>();
    let mut findings = Vec::new();

    for module in modules {
        for function in &module.functions {
            if !function.is_transaction_callable || !privileged_function(function) {
                continue;
            }

            let mut evidence = Vec::new();
            let mut guarding_types = Vec::new();

            for capability_name in &capability_names {
                if function_parameters_contain_type(&function.signature, capability_name) {
                    evidence.push(format!("requires capability parameter {capability_name}"));
                    guarding_types.push(capability_name.clone());
                }
            }

            let body = function.body.as_deref().unwrap_or("").to_ascii_lowercase();

            if body.contains("tx_context::sender") || body.contains("ctx.sender") {
                evidence.push("checks transaction sender".to_string());
            }

            if body.contains("assert!") {
                evidence
                    .push("contains assert-based authorization or invariant checks".to_string());
            }

            if evidence.is_empty() {
                evidence.push(
                    "transaction-callable privileged function without obvious guard".to_string(),
                );
            }

            findings.push(AdminControlFinding {
                function_name: function.name.clone(),
                module_name: module.name.clone(),
                qualified_name: format!("{}::{}", module.name, function.name),
                confidence: if guarding_types.is_empty() {
                    "medium"
                } else {
                    "high"
                }
                .to_string(),
                evidence,
                guarding_types,
            });
        }
    }

    findings.sort_by(|left, right| left.qualified_name.cmp(&right.qualified_name));
    findings
}

fn external_call_findings(modules: &[MoveModule]) -> Vec<ExternalCallFinding> {
    let local_modules = modules
        .iter()
        .map(|module| module.name.as_str())
        .collect::<HashSet<_>>();
    let mut findings = Vec::new();
    let mut seen = HashSet::new();

    for module in modules {
        for function in &module.functions {
            let Some(body) = function.body.as_deref() else {
                continue;
            };

            for target in call_targets(body) {
                let Some((target_module, target_function)) = target.rsplit_once("::") else {
                    continue;
                };

                if local_modules.contains(target_module) || target_module == module.name {
                    continue;
                }

                let key = format!("{}::{}->{target}", module.name, function.name);

                if !seen.insert(key) {
                    continue;
                }

                findings.push(ExternalCallFinding {
                    caller_module: module.name.clone(),
                    caller_function: function.name.clone(),
                    target_module: target_module.to_string(),
                    target_function: target_function.to_string(),
                    target,
                });
            }
        }
    }

    findings.sort_by(|left, right| {
        left.caller_module
            .cmp(&right.caller_module)
            .then_with(|| left.caller_function.cmp(&right.caller_function))
            .then_with(|| left.target.cmp(&right.target))
    });
    findings
}

fn public_package_relationships(modules: &[MoveModule]) -> Vec<PublicPackageRelationship> {
    let public_package_functions = modules
        .iter()
        .flat_map(|module| {
            module
                .functions
                .iter()
                .filter(|function| function.visibility == "public(package)")
                .map(|function| (module.name.as_str(), function.name.as_str()))
        })
        .collect::<Vec<_>>();
    let mut relationships = Vec::new();
    let mut seen = HashSet::new();

    for caller_module in modules {
        for caller in &caller_module.functions {
            let Some(body) = caller.body.as_deref() else {
                continue;
            };

            for (target_module, target_function) in &public_package_functions {
                let qualified_call = format!("{target_module}::{target_function}");
                let same_module_call = caller_module.name == *target_module
                    && body.contains(&format!("{target_function}("));

                if !body.contains(&qualified_call) && !same_module_call {
                    continue;
                }

                let key = format!(
                    "{}::{}->{}::{}",
                    caller_module.name, caller.name, target_module, target_function
                );

                if !seen.insert(key) {
                    continue;
                }

                relationships.push(PublicPackageRelationship {
                    source_module: caller_module.name.clone(),
                    source_function: caller.name.clone(),
                    target_module: (*target_module).to_string(),
                    target_function: (*target_function).to_string(),
                });
            }
        }
    }

    relationships
}

fn call_targets(source: &str) -> Vec<String> {
    source
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '(' | ')' | ',' | ';' | '{' | '}')
        })
        .filter(|token| token.contains("::"))
        .filter_map(|token| {
            let target = token
                .trim_matches(|character: char| {
                    matches!(character, '&' | '*' | '<' | '>' | ':' | ',' | ';' | '=')
                })
                .trim_end_matches('!');

            if target.matches("::").count() == 1 {
                Some(target.to_string())
            } else {
                None
            }
        })
        .collect()
}
