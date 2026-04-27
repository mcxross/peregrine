use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};
use std::{
    fs,
    path::{Path, PathBuf},
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MovePackage {
    pub name: String,
    pub path: String,
    pub manifest_path: String,
    pub surface: MovePackageSurface,
    pub modules: Vec<MoveModule>,
}

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveModule {
    pub name: String,
    pub address: Option<String>,
    pub file_path: String,
    pub structs: Vec<MoveStructSignature>,
    pub functions: Vec<MoveFunctionSignature>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructSignature {
    pub name: String,
    pub abilities: Vec<String>,
    pub fields: Vec<MoveStructField>,
    pub signature: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveStructField {
    pub name: String,
    pub type_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MoveFunctionSignature {
    pub name: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub signature: String,
    pub body: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyGraph {
    pub root: Option<String>,
    pub nodes: Vec<PackageDependencyNode>,
    pub edges: Vec<PackageDependencyEdge>,
    pub summary_path: Option<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyNode {
    pub id: String,
    pub address: Option<String>,
    pub module_count: usize,
    pub public_function_count: usize,
    pub entry_function_count: usize,
    pub is_root: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageDependencyEdge {
    pub source: String,
    pub target: String,
    pub dependency_count: usize,
    pub dependency_kind: String,
}

pub fn discover_move_project(root: &Path) -> (Vec<MovePackage>, PackageDependencyGraph) {
    let mut manifest_paths = Vec::new();

    collect_move_manifests(root, root, &mut manifest_paths);
    manifest_paths.sort();

    let packages = manifest_paths
        .into_iter()
        .filter_map(|manifest_path| build_move_package(root, &manifest_path))
        .collect::<Vec<_>>();
    let graph = build_package_dependency_graph(root, &packages);

    (packages, graph)
}

fn collect_move_manifests(root: &Path, directory: &Path, manifest_paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_manifests(root, &path, manifest_paths);
            continue;
        }

        if file_type.is_file() && entry.file_name() == "Move.toml" && path.starts_with(root) {
            manifest_paths.push(path);
        }
    }
}

fn root_package_name(packages: &[MovePackage]) -> Option<String> {
    packages
        .iter()
        .find(|move_package| move_package.path.is_empty())
        .or_else(|| packages.first())
        .map(|move_package| move_package.name.clone())
}

fn build_move_package(root: &Path, manifest_path: &Path) -> Option<MovePackage> {
    let package_root = manifest_path.parent()?;
    let manifest = fs::read_to_string(manifest_path).ok()?;
    let name = package_name(&manifest).unwrap_or_else(|| {
        package_root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("Move package")
            .to_string()
    });
    let path = relative_path(root, package_root)?;
    let manifest_path = relative_path(root, manifest_path)?;
    let mut modules = discover_modules(root, package_root);

    modules.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.file_path.cmp(&right.file_path))
    });
    let surface = package_surface(&modules);

    Some(MovePackage {
        name,
        path,
        manifest_path,
        surface,
        modules,
    })
}

fn package_surface(modules: &[MoveModule]) -> MovePackageSurface {
    let entry_function_count = modules
        .iter()
        .flat_map(|module| module.functions.iter())
        .filter(|function| function.is_transaction_callable)
        .count();
    let capability_findings = capability_findings(modules);
    let mut capability_structs = capability_findings
        .iter()
        .filter(|finding| finding.confidence != "low")
        .map(|finding| finding.qualified_name.clone())
        .collect::<Vec<_>>();
    let object_ownership_findings = object_ownership_findings(modules, &capability_structs);
    let admin_control_findings = admin_control_findings(modules, &capability_structs);
    let external_call_findings = external_call_findings(modules);
    let public_package_relationships = public_package_relationships(modules);
    let mut shared_object_structs = Vec::new();
    let shared_object_mentions = shared_object_mentions(modules);

    for module in modules {
        for move_struct in &module.structs {
            let qualified_name = format!("{}::{}", module.name, move_struct.name);

            if capability_structs.contains(&qualified_name) {
                continue;
            }

            if is_shared_object_struct(move_struct)
                && (shared_object_mentions.is_empty()
                    || shared_object_mentions.contains(&move_struct.name)
                    || shared_object_mentions.contains(&qualified_name))
            {
                shared_object_structs.push(qualified_name);
            }
        }
    }

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
        object_ownership_findings,
        admin_control_findings,
        external_call_findings,
        public_package_relationships,
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

fn capability_findings(modules: &[MoveModule]) -> Vec<CapabilityFinding> {
    let mut findings = Vec::new();

    for module in modules {
        for move_struct in &module.structs {
            let mut score = 0;
            let mut evidence = Vec::new();
            let mut protected_functions = Vec::new();
            let qualified_name = format!("{}::{}", module.name, move_struct.name);
            let mut is_used_in_transaction_callable_function = false;
            let mut guards_privileged_function = false;
            let mut is_created_and_transferred = false;
            let has_capability_name = capability_like_name(&move_struct.name);

            if struct_has_ability(move_struct, "key") {
                score += 2;
                evidence.push("struct has key ability".to_string());
            }

            if has_capability_name {
                score += 3;
                evidence.push("type name follows capability/authority naming pattern".to_string());
            }

            for function_module in modules {
                for function in &function_module.functions {
                    let parameter_uses_type =
                        function_parameters_contain_type(&function.signature, &move_struct.name)
                            || function_parameters_contain_type(
                                &function.signature,
                                &qualified_name,
                            );

                    if parameter_uses_type && function.is_transaction_callable {
                        is_used_in_transaction_callable_function = true;
                        evidence.push(format!(
                            "used as a parameter in transaction-callable function {}::{}",
                            function_module.name, function.name
                        ));
                    }

                    if parameter_uses_type && privileged_function(function) {
                        guards_privileged_function = true;
                        evidence.push(format!(
                            "guards privileged-looking function {}::{}",
                            function_module.name, function.name
                        ));
                        protected_functions
                            .push(format!("{}::{}", function_module.name, function.name));
                    }

                    if created_and_transferred(function, &move_struct.name) {
                        is_created_and_transferred = true;
                        evidence.push(format!(
                            "created and transferred in {}::{}",
                            function_module.name, function.name
                        ));
                    }
                }
            }

            if is_used_in_transaction_callable_function {
                score += 2;
            }

            if guards_privileged_function {
                score += 2;
            }

            if is_created_and_transferred {
                score += 2;
            }

            evidence.sort();
            evidence.dedup();
            protected_functions.sort();
            protected_functions.dedup();

            let confidence = if has_capability_name {
                capability_confidence(score)
            } else {
                "low"
            };

            if confidence == "low" && evidence.is_empty() {
                continue;
            }

            findings.push(CapabilityFinding {
                type_name: move_struct.name.clone(),
                module_name: module.name.clone(),
                qualified_name,
                confidence: confidence.to_string(),
                evidence,
                protected_functions,
            });
        }
    }

    findings.sort_by(|left, right| {
        confidence_rank(&right.confidence)
            .cmp(&confidence_rank(&left.confidence))
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
    });

    findings
}

fn capability_confidence(score: i32) -> &'static str {
    if score >= 7 {
        "high"
    } else if score >= 4 {
        "medium"
    } else {
        "low"
    }
}

fn confidence_rank(confidence: &str) -> u8 {
    match confidence {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

fn capability_like_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    name.ends_with("cap")
        || name.ends_with("capability")
        || name.contains("_cap")
        || name.contains("admin")
        || name.contains("authority")
        || name.contains("owner")
        || name.contains("publisher")
        || name.contains("operator")
        || name.contains("guardian")
}

fn struct_has_ability(move_struct: &MoveStructSignature, ability: &str) -> bool {
    move_struct
        .abilities
        .iter()
        .any(|candidate| candidate == ability)
}

fn is_shared_object_struct(move_struct: &MoveStructSignature) -> bool {
    struct_has_ability(move_struct, "key") && !capability_like_name(&move_struct.name)
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

fn created_and_transferred(function: &MoveFunctionSignature, type_name: &str) -> bool {
    let Some(body) = function.body.as_deref() else {
        return false;
    };
    let lower_body = body.to_ascii_lowercase();

    body.contains(type_name)
        && lower_body.contains("object::new")
        && (lower_body.contains("transfer::transfer")
            || lower_body.contains("transfer::public_transfer")
            || lower_body.contains("share_object"))
}

fn object_ownership_findings(
    modules: &[MoveModule],
    capability_structs: &[String],
) -> Vec<ObjectOwnershipFinding> {
    let key_structs = key_structs(modules);
    let mut findings = Vec::new();

    for (module_name, move_struct) in &key_structs {
        let qualified_name = format!("{module_name}::{}", move_struct.name);

        if capability_structs.contains(&qualified_name) {
            continue;
        }

        for kind in ["shared", "addressOwned", "immutable", "party"] {
            let (evidence, related_functions) =
                ownership_evidence(modules, &move_struct.name, &qualified_name, kind);

            if evidence.is_empty() {
                continue;
            }

            findings.push(ObjectOwnershipFinding {
                type_name: move_struct.name.clone(),
                module_name: module_name.clone(),
                qualified_name: qualified_name.clone(),
                ownership_kind: kind.to_string(),
                confidence: if related_functions.is_empty() {
                    "medium".to_string()
                } else {
                    "high".to_string()
                },
                evidence,
                related_functions,
                wrapped_types: Vec::new(),
            });
        }
    }

    findings.extend(wrapped_object_findings(
        modules,
        &key_structs,
        capability_structs,
    ));
    findings.sort_by(|left, right| {
        left.ownership_kind
            .cmp(&right.ownership_kind)
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
    });
    findings
}

fn key_structs(modules: &[MoveModule]) -> Vec<(String, &MoveStructSignature)> {
    modules
        .iter()
        .flat_map(|module| {
            module
                .structs
                .iter()
                .filter(|move_struct| struct_has_ability(move_struct, "key"))
                .map(|move_struct| (module.name.clone(), move_struct))
        })
        .collect()
}

fn ownership_evidence(
    modules: &[MoveModule],
    type_name: &str,
    qualified_name: &str,
    kind: &str,
) -> (Vec<String>, Vec<String>) {
    let mut evidence = Vec::new();
    let mut related_functions = Vec::new();

    for module in modules {
        for function in &module.functions {
            let Some(body) = function.body.as_deref() else {
                continue;
            };

            let lower_body = body.to_ascii_lowercase();
            let references_type = type_reference_matches(body, type_name)
                || body.contains(qualified_name)
                || body_constructs_type(body, type_name)
                || function_returns_type(&function.signature, type_name)
                || function_returns_type(&function.signature, qualified_name);
            let matched = match kind {
                "shared" => references_type && lower_body.contains("share_object"),
                "addressOwned" => {
                    (references_type
                        && (lower_body.contains("transfer::transfer")
                            || lower_body.contains("transfer::public_transfer")
                            || lower_body.contains("public_transfer")))
                        || (function.is_transaction_callable
                            && function_returns_type(&function.signature, type_name))
                        || (function.is_transaction_callable
                            && function_returns_type(&function.signature, qualified_name))
                }
                "immutable" => references_type && lower_body.contains("freeze_object"),
                "party" => {
                    references_type
                        && (lower_body.contains("party_transfer") || lower_body.contains("party::"))
                }
                _ => false,
            };

            if matched {
                evidence.push(ownership_evidence_label(
                    kind,
                    module,
                    function,
                    type_name,
                    qualified_name,
                ));
                related_functions.push(format!("{}::{}", module.name, function.name));
            }
        }
    }

    evidence.sort();
    evidence.dedup();
    related_functions.sort();
    related_functions.dedup();
    (evidence, related_functions)
}

fn ownership_evidence_label(
    kind: &str,
    module: &MoveModule,
    function: &MoveFunctionSignature,
    type_name: &str,
    qualified_name: &str,
) -> String {
    match kind {
        "addressOwned"
            if function.is_transaction_callable
                && (function_returns_type(&function.signature, type_name)
                    || function_returns_type(&function.signature, qualified_name)) =>
        {
            format!(
                "owned object returned from transaction-callable {}::{}",
                module.name, function.name
            )
        }
        "shared" => format!(
            "object shared via transfer::share_object in {}::{}",
            module.name, function.name
        ),
        "immutable" => format!(
            "object frozen via transfer::freeze_object in {}::{}",
            module.name, function.name
        ),
        "party" => format!(
            "object moved through party transfer API in {}::{}",
            module.name, function.name
        ),
        _ => format!(
            "{kind} ownership evidence in {}::{}",
            module.name, function.name
        ),
    }
}

fn body_constructs_type(body: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);
    body.contains(&format!("{short_name} {{")) || body.contains(&format!("{short_name}<"))
}

fn function_returns_type(signature: &str, type_name: &str) -> bool {
    if type_name.is_empty() {
        return false;
    }

    let Some(close_parameters) = signature.rfind(')') else {
        return false;
    };

    let after_parameters = signature[close_parameters + 1..].trim_start();
    let Some(return_type) = after_parameters.strip_prefix(':') else {
        return false;
    };

    type_reference_matches(return_type, type_name)
}

fn wrapped_object_findings(
    modules: &[MoveModule],
    key_structs: &[(String, &MoveStructSignature)],
    capability_structs: &[String],
) -> Vec<ObjectOwnershipFinding> {
    let key_names = key_structs
        .iter()
        .map(|(_, move_struct)| move_struct.name.clone())
        .collect::<HashSet<_>>();
    let mut findings = Vec::new();

    for module in modules {
        for wrapper in &module.structs {
            let qualified_name = format!("{}::{}", module.name, wrapper.name);

            if capability_structs.contains(&qualified_name) {
                continue;
            }

            let wrapped_types = wrapper
                .fields
                .iter()
                .filter_map(|field| {
                    key_names
                        .iter()
                        .find(|key_name| type_reference_matches(&field.type_name, key_name))
                        .cloned()
                })
                .collect::<HashSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();

            if wrapped_types.is_empty() {
                continue;
            }

            findings.push(ObjectOwnershipFinding {
                type_name: wrapper.name.clone(),
                module_name: module.name.clone(),
                qualified_name,
                ownership_kind: "wrapped".to_string(),
                confidence: "high".to_string(),
                evidence: vec!["struct stores another key object type as a field".to_string()],
                related_functions: Vec::new(),
                wrapped_types,
            });
        }
    }

    findings
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

fn shared_object_mentions(modules: &[MoveModule]) -> HashSet<String> {
    let mut mentions = HashSet::new();

    for function in modules.iter().flat_map(|module| module.functions.iter()) {
        let Some(body) = function.body.as_deref() else {
            continue;
        };

        if !body.contains("share_object") {
            continue;
        }

        for module in modules {
            for move_struct in &module.structs {
                if body.contains(&move_struct.name) {
                    mentions.insert(move_struct.name.clone());
                    mentions.insert(format!("{}::{}", module.name, move_struct.name));
                }
            }
        }
    }

    mentions
}

fn discover_modules(root: &Path, package_root: &Path) -> Vec<MoveModule> {
    let sources = package_root.join("sources");
    let mut modules = Vec::new();

    collect_move_modules(root, &sources, &mut modules);
    modules
}

fn collect_move_modules(root: &Path, directory: &Path, modules: &mut Vec<MoveModule>) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_move_modules(root, &path, modules);
            continue;
        }

        if !file_type.is_file() || !is_move_file(&path) {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let Some(module) = parse_module_declaration(&source, root, &path) else {
            continue;
        };

        modules.push(module);
    }
}

fn parse_module_declaration(source: &str, root: &Path, path: &Path) -> Option<MoveModule> {
    for line in source.lines() {
        let line = line.split("//").next().unwrap_or("").trim();
        let Some(declaration) = line
            .strip_prefix("module ")
            .or_else(|| line.strip_prefix("public module "))
        else {
            continue;
        };
        let qualified_name = declaration
            .split(|character: char| character == '{' || character.is_whitespace())
            .next()?
            .trim_end_matches(';');
        let (address, name) = match qualified_name.split_once("::") {
            Some((address, name)) => (Some(address.to_string()), name.to_string()),
            None => (None, qualified_name.to_string()),
        };

        if name.is_empty() {
            return None;
        }

        return Some(MoveModule {
            name,
            address,
            file_path: relative_path(root, path)?,
            structs: collect_struct_signatures(source),
            functions: collect_function_signatures(source),
        });
    }

    None
}

fn collect_struct_signatures(source: &str) -> Vec<MoveStructSignature> {
    let mut structs = Vec::new();
    let mut current = String::new();
    let mut brace_depth = 0_i32;
    let mut has_body = false;
    let mut is_collecting = false;

    for raw_line in source.lines() {
        let line = raw_line.split("//").next().unwrap_or("").trim();

        if line.is_empty() {
            continue;
        }

        if !is_collecting && !line_contains_struct_declaration(line) {
            continue;
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(line);
        is_collecting = true;

        if line.contains('{') {
            has_body = true;
        }

        brace_depth += brace_balance(line);

        if !has_body && !line.ends_with(';') {
            continue;
        }

        if has_body && brace_depth > 0 {
            continue;
        }

        if let Some(signature) = parse_struct_signature(&current) {
            structs.push(signature);
        }

        current.clear();
        brace_depth = 0;
        has_body = false;
        is_collecting = false;
    }

    structs
}

fn line_contains_struct_declaration(line: &str) -> bool {
    line == "struct"
        || line.starts_with("struct ")
        || line.contains(" struct ")
        || line.starts_with("public struct ")
        || line.starts_with("public(package) struct ")
        || line.starts_with("public(friend) struct ")
        || line.starts_with("native struct ")
        || line.contains(" native struct ")
}

fn parse_struct_signature(source: &str) -> Option<MoveStructSignature> {
    let signature = source
        .split('{')
        .next()
        .unwrap_or(source)
        .split(';')
        .next()
        .unwrap_or(source)
        .trim()
        .to_string();
    let struct_index = signature.find("struct ")?;
    let after_struct = signature[struct_index + 7..].trim();
    let name = after_struct
        .split(|character: char| character == '<' || character == '{' || character.is_whitespace())
        .next()?
        .to_string();

    if name.is_empty() {
        return None;
    }

    Some(MoveStructSignature {
        name,
        abilities: struct_abilities(&signature),
        fields: struct_fields(source),
        signature,
    })
}

fn struct_fields(source: &str) -> Vec<MoveStructField> {
    let Some(start) = source.find('{') else {
        return Vec::new();
    };
    let Some(end) = source.rfind('}') else {
        return Vec::new();
    };
    let body = &source[start + 1..end];

    body.lines()
        .filter_map(|line| {
            let line = line
                .split("//")
                .next()
                .unwrap_or("")
                .trim()
                .trim_end_matches(',');

            if line.is_empty() {
                return None;
            }

            let (name, type_name) = line.split_once(':')?;
            Some(MoveStructField {
                name: name.trim().to_string(),
                type_name: type_name.trim().to_string(),
            })
        })
        .collect()
}

fn struct_abilities(signature: &str) -> Vec<String> {
    let Some((_, after_has)) = signature.split_once(" has ") else {
        return Vec::new();
    };
    let abilities_source = after_has
        .split(" where ")
        .next()
        .unwrap_or(after_has)
        .trim();

    abilities_source
        .split(',')
        .filter_map(|ability| {
            let ability = ability
                .trim()
                .trim_end_matches('{')
                .trim_end_matches(';')
                .trim();

            if ability.is_empty() {
                None
            } else {
                Some(ability.to_string())
            }
        })
        .collect()
}

fn collect_function_signatures(source: &str) -> Vec<MoveFunctionSignature> {
    let mut functions = Vec::new();
    let mut current = String::new();
    let mut brace_depth = 0_i32;
    let mut has_body = false;
    let mut is_collecting = false;

    for raw_line in source.lines() {
        let code_line = raw_line.split("//").next().unwrap_or("").trim();

        if code_line.is_empty() {
            continue;
        }

        if !is_collecting && !line_contains_function_declaration(code_line) {
            continue;
        }

        if !current.is_empty() {
            current.push('\n');
        }
        current.push_str(raw_line.trim_end());
        is_collecting = true;

        let line_balance = brace_balance(code_line);

        if code_line.contains('{') {
            has_body = true;
        }

        brace_depth += line_balance;

        if !has_body && !code_line.ends_with(';') {
            continue;
        }

        if has_body && brace_depth > 0 {
            continue;
        }

        if let Some(signature) = parse_function_signature(&current) {
            functions.push(signature);
        }

        current.clear();
        brace_depth = 0;
        has_body = false;
        is_collecting = false;
    }

    functions
}

fn brace_balance(line: &str) -> i32 {
    line.chars().fold(0, |balance, character| match character {
        '{' => balance + 1,
        '}' => balance - 1,
        _ => balance,
    })
}

fn line_contains_function_declaration(line: &str) -> bool {
    line.split(|character: char| {
        character.is_whitespace()
            || matches!(
                character,
                '(' | ')' | '<' | '>' | ',' | ':' | ';' | '{' | '}'
            )
    })
    .any(|token| token == "fun")
}

fn parse_function_signature(source: &str) -> Option<MoveFunctionSignature> {
    let signature = source
        .split('{')
        .next()
        .unwrap_or(source)
        .trim()
        .trim_end_matches(';')
        .to_string();
    let fun_index = signature.find("fun ")?;
    let prefix = signature[..fun_index].trim();
    let after_fun = signature[fun_index + 4..].trim();
    let name = after_fun
        .split(|character: char| character == '<' || character == '(' || character.is_whitespace())
        .next()?
        .to_string();

    if name.is_empty() {
        return None;
    }

    let visibility = function_visibility(prefix);
    let is_entry = function_has_entry_modifier(prefix);

    Some(MoveFunctionSignature {
        name,
        is_transaction_callable: is_entry || visibility == "public",
        visibility,
        is_entry,
        signature,
        body: function_body(source),
    })
}

fn function_has_entry_modifier(prefix: &str) -> bool {
    prefix
        .split(|character: char| {
            character.is_whitespace()
                || matches!(character, '(' | ')' | '<' | '>' | ',' | ':' | ';')
        })
        .any(|token| token == "entry")
}

fn function_body(source: &str) -> Option<String> {
    let body = source.trim();

    if body.contains('{') {
        Some(body.to_string())
    } else {
        None
    }
}

fn function_visibility(prefix: &str) -> String {
    if prefix.contains("public(friend)") {
        "public(friend)".to_string()
    } else if prefix.contains("public(package)") {
        "public(package)".to_string()
    } else if prefix.split_whitespace().any(|token| token == "public") {
        "public".to_string()
    } else {
        "private".to_string()
    }
}

fn package_name(manifest: &str) -> Option<String> {
    let mut in_package_section = false;

    for line in manifest.lines() {
        let line = line.split('#').next().unwrap_or("").trim();

        if line.starts_with('[') && line.ends_with(']') {
            in_package_section = line == "[package]";
            continue;
        }

        if !in_package_section {
            continue;
        }

        let Some((key, value)) = line.split_once('=') else {
            continue;
        };

        if key.trim() != "name" {
            continue;
        }

        return Some(
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string(),
        );
    }

    None
}

fn build_package_dependency_graph(root: &Path, packages: &[MovePackage]) -> PackageDependencyGraph {
    let Some(summary_location) = resolve_summary_location(root, packages) else {
        return empty_dependency_graph(root_package_name(packages).as_deref());
    };

    let address_mapping = read_address_mapping(&summary_location.path.join("address_mapping.json"));
    let mut aggregate = DependencyAggregate::default();

    collect_summary_dependencies(&summary_location.path, &mut aggregate);

    let root = Some(summary_location.root_package.clone())
        .or_else(|| aggregate.module_counts.keys().next().cloned());
    let Some(root_id) = root else {
        return PackageDependencyGraph {
            root: None,
            nodes: Vec::new(),
            edges: Vec::new(),
            summary_path: Some(summary_location.relative_path),
        };
    };

    let reachable = reachable_packages(&root_id, &aggregate.edges);
    let mut nodes = reachable
        .iter()
        .map(|id| PackageDependencyNode {
            id: id.clone(),
            address: address_mapping.get(id).cloned(),
            module_count: *aggregate.module_counts.get(id).unwrap_or(&0),
            public_function_count: *aggregate.public_function_counts.get(id).unwrap_or(&0),
            entry_function_count: *aggregate.entry_function_counts.get(id).unwrap_or(&0),
            is_root: id == &root_id,
        })
        .collect::<Vec<_>>();
    nodes.sort_by(|left, right| {
        right
            .is_root
            .cmp(&left.is_root)
            .then_with(|| left.id.cmp(&right.id))
    });

    let mut edges = aggregate
        .edges
        .into_iter()
        .filter(|((source, target), _)| reachable.contains(source) && reachable.contains(target))
        .map(
            |((source, target), dependency_count)| PackageDependencyEdge {
                source,
                target,
                dependency_count,
                dependency_kind: "Immediate module dependency".to_string(),
            },
        )
        .collect::<Vec<_>>();
    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then_with(|| left.target.cmp(&right.target))
    });

    PackageDependencyGraph {
        root: Some(root_id),
        nodes,
        edges,
        summary_path: Some(summary_location.relative_path),
    }
}

struct SummaryLocation {
    path: PathBuf,
    relative_path: String,
    root_package: String,
}

fn resolve_summary_location(root: &Path, packages: &[MovePackage]) -> Option<SummaryLocation> {
    let root_summary = root.join("package_summaries");

    if root_summary.is_dir() {
        return Some(SummaryLocation {
            relative_path: relative_path(root, &root_summary)?,
            path: root_summary,
            root_package: root_package_name(packages)?,
        });
    }

    packages
        .iter()
        .filter_map(|move_package| {
            let package_summary = root.join(&move_package.path).join("package_summaries");

            if !package_summary.is_dir() {
                return None;
            }

            Some(SummaryLocation {
                relative_path: relative_path(root, &package_summary)?,
                path: package_summary,
                root_package: move_package.name.clone(),
            })
        })
        .min_by(|left, right| {
            left.relative_path
                .matches('/')
                .count()
                .cmp(&right.relative_path.matches('/').count())
                .then_with(|| left.relative_path.cmp(&right.relative_path))
        })
}

fn empty_dependency_graph(root: Option<&str>) -> PackageDependencyGraph {
    PackageDependencyGraph {
        root: root.map(ToOwned::to_owned),
        nodes: root
            .map(|id| {
                vec![PackageDependencyNode {
                    id: id.to_string(),
                    address: None,
                    module_count: 0,
                    public_function_count: 0,
                    entry_function_count: 0,
                    is_root: true,
                }]
            })
            .unwrap_or_default(),
        edges: Vec::new(),
        summary_path: None,
    }
}

#[derive(Default)]
struct DependencyAggregate {
    module_counts: HashMap<String, usize>,
    public_function_counts: HashMap<String, usize>,
    entry_function_counts: HashMap<String, usize>,
    edges: HashMap<(String, String), usize>,
}

#[derive(Deserialize)]
struct ModuleSummary {
    id: ModuleId,
    #[serde(default)]
    immediate_dependencies: Vec<ModuleId>,
    #[serde(default)]
    functions: HashMap<String, SummaryFunction>,
}

#[derive(Deserialize)]
struct ModuleId {
    address: String,
    #[allow(dead_code)]
    name: String,
}

#[derive(Deserialize)]
struct SummaryFunction {
    #[serde(default)]
    visibility: Option<serde_json::Value>,
    #[serde(default)]
    entry: bool,
}

fn collect_summary_dependencies(directory: &Path, aggregate: &mut DependencyAggregate) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if file_type.is_dir() {
            collect_summary_dependencies(&path, aggregate);
            continue;
        }

        if !file_type.is_file()
            || path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "address_mapping.json")
            || !path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("json"))
        {
            continue;
        }

        let Ok(source) = fs::read_to_string(&path) else {
            continue;
        };
        let Ok(summary) = serde_json::from_str::<ModuleSummary>(&source) else {
            continue;
        };

        *aggregate
            .module_counts
            .entry(summary.id.address.clone())
            .or_default() += 1;

        for function in summary.functions.values() {
            if function.entry {
                *aggregate
                    .entry_function_counts
                    .entry(summary.id.address.clone())
                    .or_default() += 1;
            }

            if is_public_visibility(function.visibility.as_ref()) {
                *aggregate
                    .public_function_counts
                    .entry(summary.id.address.clone())
                    .or_default() += 1;
            }
        }

        for dependency in summary.immediate_dependencies {
            if dependency.address == summary.id.address {
                continue;
            }

            *aggregate
                .edges
                .entry((summary.id.address.clone(), dependency.address))
                .or_default() += 1;
        }
    }
}

fn is_public_visibility(visibility: Option<&serde_json::Value>) -> bool {
    match visibility {
        Some(serde_json::Value::String(value)) => value.starts_with("Public"),
        Some(serde_json::Value::Object(value)) => value.contains_key("Public"),
        _ => false,
    }
}

fn reachable_packages(root: &str, edges: &HashMap<(String, String), usize>) -> HashSet<String> {
    let mut reachable = HashSet::from([root.to_string()]);
    let mut queue = VecDeque::from([root.to_string()]);

    while let Some(source) = queue.pop_front() {
        for ((edge_source, edge_target), _) in edges {
            if edge_source != &source || reachable.contains(edge_target) {
                continue;
            }

            reachable.insert(edge_target.clone());
            queue.push_back(edge_target.clone());
        }
    }

    reachable
}

fn read_address_mapping(path: &Path) -> HashMap<String, String> {
    let Ok(source) = fs::read_to_string(path) else {
        return HashMap::new();
    };

    serde_json::from_str(&source).unwrap_or_default()
}

fn is_move_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("move"))
}

fn relative_path(root: &Path, path: &Path) -> Option<String> {
    Some(
        path.strip_prefix(root)
            .ok()?
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?
            .join("/"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn package_surface_detects_sui_object_ownership_patterns() {
        let root = Path::new("/workspace");
        let vault = parse_module_declaration(
            r#"
module savings_personal::vault {
    public struct SavingsAdminCap has key, store { id: UID }
    public struct SavingsTreasury has key, store { id: UID }
    public struct SavingsVault<phantom Asset> has key, store { id: UID }

    public entry fun init(ctx: &mut TxContext) {
        transfer::transfer(SavingsAdminCap { id: object::new(ctx) }, ctx.sender());
        transfer::share_object(SavingsTreasury { id: object::new(ctx) });
        let vault = SavingsVault<u64> { id: object::new(ctx) };
        transfer::share_object(vault);
    }

    public entry fun update_config(cap: &SavingsAdminCap, vault: &mut SavingsVault<u64>) {
        assert!(true, 0);
    }
}
"#,
            root,
            Path::new("/workspace/sources/vault.move"),
        )
        .expect("vault module should parse");
        let savings = parse_module_declaration(
            r#"
module savings_personal::savings_personal {
    public struct VaultReceipt has key, store { id: UID }
    public struct ReceiptWrapper has key, store { id: UID, receipt: VaultReceipt }

    public fun register_account(ctx: &mut TxContext): VaultReceipt {
        VaultReceipt { id: object::new(ctx) }
    }

    public(package) fun borrow_receipt(receipt: &VaultReceipt): ID {
        object::id(receipt)
    }
}
"#,
            root,
            Path::new("/workspace/sources/savings_personal.move"),
        )
        .expect("savings module should parse");

        let surface = package_surface(&[vault, savings]);

        assert!(surface.entry_function_count >= 3);
        assert!(surface.capability_count >= 1);
        assert!(surface.shared_object_count >= 2);
        assert!(surface.address_owned_object_count >= 1);
        assert!(surface.wrapped_object_count >= 1);
        assert!(surface.admin_control_count >= 1);
    }
}
