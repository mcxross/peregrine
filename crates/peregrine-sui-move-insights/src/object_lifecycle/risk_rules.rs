fn lifecycle_risks(
    qualified_name: &str,
    type_name: &str,
    is_capability_like: bool,
    stages: &[ObjectLifecycleStage],
    functions: &BTreeMap<String, FunctionLookup<'_>>,
) -> Vec<ObjectLifecycleRisk> {
    let mut risks = Vec::new();

    if has_stage(stages, "created") && !has_stage(stages, "deleted") {
        risks.push(ObjectLifecycleRisk {
            kind: "createdWithoutDeletion".to_string(),
            severity: "medium".to_string(),
            message: format!("{qualified_name} can be created, but no delete path was detected."),
            evidence: stage_evidence(stages, "created"),
            functions: stage_functions(stages, "created"),
        });
    }

    let unguarded_delete = externally_reachable_unguarded(stages, functions, &["deleted"]);
    if !unguarded_delete.is_empty() {
        risks.push(ObjectLifecycleRisk {
            kind: "deletionTooAccessible".to_string(),
            severity: "high".to_string(),
            message: format!(
                "{qualified_name} has a delete path reachable without an obvious guard."
            ),
            evidence: stage_evidence(stages, "deleted"),
            functions: unguarded_delete,
        });
    }

    let unguarded_transfer =
        externally_reachable_unguarded(stages, functions, &["transferred", "shared", "party"]);
    if !unguarded_transfer.is_empty() {
        risks.push(ObjectLifecycleRisk {
            kind: "unguardedOwnershipChange".to_string(),
            severity: "high".to_string(),
            message: format!(
                "{qualified_name} changes ownership in reachable functions without an obvious guard."
            ),
            evidence: combined_stage_evidence(stages, &["transferred", "shared", "party"]),
            functions: unguarded_transfer,
        });
    }

    if receipt_or_position_like(type_name) && !has_stage(stages, "deleted") {
        risks.push(ObjectLifecycleRisk {
            kind: "longLivedReceiptOrPosition".to_string(),
            severity: "medium".to_string(),
            message: format!(
                "{qualified_name} looks like a receipt or position and has no consume/delete path."
            ),
            evidence: vec!["type name follows receipt/position naming pattern".to_string()],
            functions: stage_functions(stages, "created"),
        });
    }

    if is_capability_like && has_any_stage(stages, &["owned", "transferred", "shared", "wrapped"]) {
        let leak_functions = externally_reachable_unguarded(
            stages,
            functions,
            &["owned", "transferred", "shared", "wrapped"],
        );

        if !leak_functions.is_empty() || has_stage(stages, "shared") {
            risks.push(ObjectLifecycleRisk {
                kind: "privilegedObjectLeak".to_string(),
                severity: if has_stage(stages, "shared") {
                    "high"
                } else {
                    "medium"
                }
                .to_string(),
                message: format!(
                    "{qualified_name} is capability-like and can leave its guarded lifecycle path."
                ),
                evidence: combined_stage_evidence(
                    stages,
                    &["owned", "transferred", "shared", "wrapped"],
                ),
                functions: leak_functions,
            });
        }
    }

    risks.sort_by(|left, right| {
        severity_rank(&right.severity)
            .cmp(&severity_rank(&left.severity))
            .then_with(|| left.kind.cmp(&right.kind))
    });
    risks
}

fn externally_reachable_unguarded(
    stages: &[ObjectLifecycleStage],
    functions: &BTreeMap<String, FunctionLookup<'_>>,
    stage_kinds: &[&str],
) -> Vec<ObjectLifecycleFunctionRef> {
    let mut refs = stage_kinds
        .iter()
        .flat_map(|kind| stage_functions(stages, kind))
        .filter(|function_ref| {
            externally_reachable(function_ref)
                && functions
                    .get(&function_ref.qualified_name)
                    .is_none_or(|lookup| !function_has_guard(lookup.function))
        })
        .collect::<Vec<_>>();

    sort_function_refs(&mut refs);
    refs.dedup();
    refs
}

fn externally_reachable(function_ref: &ObjectLifecycleFunctionRef) -> bool {
    function_ref.is_transaction_callable
        || function_ref.is_entry
        || function_ref.visibility == "public"
        || function_ref.visibility == "public(package)"
}

fn function_has_guard(function: &MoveFunctionSignature) -> bool {
    let parameters = function_parameters(&function.signature)
        .unwrap_or("")
        .to_ascii_lowercase();
    let body = function.body.as_deref().unwrap_or("").to_ascii_lowercase();

    parameters.contains("cap")
        || parameters.contains("admin")
        || parameters.contains("treasury")
        || parameters.contains("authority")
        || body.contains("assert!")
        || body.contains("tx_context::sender")
        || body.contains("ctx.sender")
        || body.contains("sender(")
        || body.contains("assert_sender")
        || body.contains("authorize")
        || body.contains("ensure_")
        || body.contains("check_")
}

fn has_stage(stages: &[ObjectLifecycleStage], kind: &str) -> bool {
    stages.iter().any(|stage| stage.kind == kind)
}

fn has_any_stage(stages: &[ObjectLifecycleStage], kinds: &[&str]) -> bool {
    kinds.iter().any(|kind| has_stage(stages, kind))
}

fn stage_functions(stages: &[ObjectLifecycleStage], kind: &str) -> Vec<ObjectLifecycleFunctionRef> {
    stages
        .iter()
        .find(|stage| stage.kind == kind)
        .map(|stage| stage.functions.clone())
        .unwrap_or_default()
}

fn stage_evidence(stages: &[ObjectLifecycleStage], kind: &str) -> Vec<String> {
    stages
        .iter()
        .find(|stage| stage.kind == kind)
        .map(|stage| stage.evidence.clone())
        .unwrap_or_default()
}

fn combined_stage_evidence(stages: &[ObjectLifecycleStage], kinds: &[&str]) -> Vec<String> {
    let mut evidence = kinds
        .iter()
        .flat_map(|kind| stage_evidence(stages, kind))
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    evidence.sort();
    evidence
}

fn sort_function_refs(functions: &mut [ObjectLifecycleFunctionRef]) {
    functions.sort_by(|left, right| {
        right
            .direct
            .cmp(&left.direct)
            .then_with(|| left.file_path.cmp(&right.file_path))
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
            .then_with(|| left.call_path.cmp(&right.call_path))
    });
}

fn stage_rank(kind: &str) -> usize {
    STAGE_ORDER
        .iter()
        .position(|candidate| *candidate == kind)
        .unwrap_or(STAGE_ORDER.len())
}

fn severity_rank(severity: &str) -> u8 {
    match severity {
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}
