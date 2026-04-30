use super::{
    AdminControlFinding, CapabilityFinding, ExternalCallFinding, MoveFunctionSignature, MoveModule,
    MovePackageSurface, MoveStructSignature, ObjectOwnershipFinding, PublicPackageRelationship,
};
use std::collections::HashSet;

pub(crate) fn package_surface(modules: &[MoveModule]) -> MovePackageSurface {
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
