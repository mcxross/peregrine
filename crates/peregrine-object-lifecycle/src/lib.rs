use peregrine_move_model::{MoveFunctionSignature, MoveModule, MoveStructSignature};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const CALL_GRAPH_DEPTH: usize = 5;
const STAGE_ORDER: &[&str] = &[
    "created",
    "owned",
    "mutated",
    "transferred",
    "shared",
    "wrapped",
    "immutable",
    "party",
    "deleted",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleMap {
    pub type_name: String,
    pub module_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub abilities: Vec<String>,
    pub is_capability_like: bool,
    pub stages: Vec<ObjectLifecycleStage>,
    pub touched_by: Vec<ObjectLifecycleFunctionRef>,
    pub risks: Vec<ObjectLifecycleRisk>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleStage {
    pub kind: String,
    pub functions: Vec<ObjectLifecycleFunctionRef>,
    pub evidence: Vec<String>,
}

#[derive(Serialize, Clone, Eq, PartialEq, Ord, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleFunctionRef {
    pub module_name: String,
    pub function_name: String,
    pub qualified_name: String,
    pub file_path: String,
    pub visibility: String,
    pub is_entry: bool,
    pub is_transaction_callable: bool,
    pub direct: bool,
    pub call_path: Vec<String>,
    pub evidence: Vec<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ObjectLifecycleRisk {
    pub kind: String,
    pub severity: String,
    pub message: String,
    pub evidence: Vec<String>,
    pub functions: Vec<ObjectLifecycleFunctionRef>,
}

#[derive(Clone, Copy)]
struct FunctionLookup<'a> {
    module: &'a MoveModule,
    function: &'a MoveFunctionSignature,
}

struct ObjectCandidate<'a> {
    module: &'a MoveModule,
    move_struct: &'a MoveStructSignature,
    qualified_name: String,
}

#[derive(Clone)]
struct DirectEvent {
    object_key: String,
    stage: String,
    function_key: String,
    evidence: String,
}

pub fn object_lifecycle_maps(
    modules: &[MoveModule],
    capability_structs: &[String],
) -> Vec<ObjectLifecycleMap> {
    let objects = key_object_candidates(modules);
    let functions = function_index(modules);
    let reverse_call_graph = reverse_call_graph(&functions);
    let direct_events = direct_lifecycle_events(&objects, &functions);
    let wrapper_evidence = wrapper_evidence(&objects);
    let capability_set = capability_structs.iter().cloned().collect::<BTreeSet<_>>();
    let mut maps = Vec::new();

    for object in objects {
        let mut stage_functions: BTreeMap<String, Vec<ObjectLifecycleFunctionRef>> =
            BTreeMap::new();
        let mut stage_evidence: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

        for (stage, evidence) in wrapper_evidence
            .get(&object.qualified_name)
            .into_iter()
            .flat_map(|evidence| evidence.iter())
        {
            stage_evidence
                .entry(stage.clone())
                .or_default()
                .insert(evidence.clone());
        }

        for event in direct_events
            .iter()
            .filter(|event| event.object_key == object.qualified_name)
        {
            let Some(lookup) = functions.get(&event.function_key) else {
                continue;
            };

            push_stage_function(
                &mut stage_functions,
                &event.stage,
                lifecycle_function_ref(*lookup, true, Vec::new(), vec![event.evidence.clone()]),
            );
            stage_evidence
                .entry(event.stage.clone())
                .or_default()
                .insert(event.evidence.clone());

            for indirect in indirect_callers(&event.function_key, &reverse_call_graph) {
                let Some(lookup) = functions.get(&indirect.caller_key) else {
                    continue;
                };
                let evidence = format!("calls {}", event.function_key);

                push_stage_function(
                    &mut stage_functions,
                    &event.stage,
                    lifecycle_function_ref(
                        *lookup,
                        false,
                        indirect.call_path,
                        vec![evidence.clone()],
                    ),
                );
                stage_evidence
                    .entry(event.stage.clone())
                    .or_default()
                    .insert(evidence);
            }
        }

        let mut stages = stage_evidence
            .into_iter()
            .map(|(kind, evidence)| {
                let mut functions = stage_functions.remove(&kind).unwrap_or_default();
                sort_function_refs(&mut functions);

                ObjectLifecycleStage {
                    kind,
                    functions,
                    evidence: evidence.into_iter().collect(),
                }
            })
            .collect::<Vec<_>>();

        stages.sort_by_key(|stage| stage_rank(&stage.kind));

        let mut touched_by = stages
            .iter()
            .flat_map(|stage| stage.functions.iter().cloned())
            .collect::<Vec<_>>();
        sort_function_refs(&mut touched_by);
        touched_by.dedup();

        let is_capability_like = capability_set.contains(&object.qualified_name)
            || capability_like_name(&object.move_struct.name);
        let risks = lifecycle_risks(
            &object.qualified_name,
            &object.move_struct.name,
            is_capability_like,
            &stages,
            &functions,
        );

        maps.push(ObjectLifecycleMap {
            type_name: object.move_struct.name.clone(),
            module_name: object.module.name.clone(),
            qualified_name: object.qualified_name,
            file_path: object.module.file_path.clone(),
            abilities: object.move_struct.abilities.clone(),
            is_capability_like,
            stages,
            touched_by,
            risks,
        });
    }

    maps.sort_by(|left, right| {
        left.file_path
            .cmp(&right.file_path)
            .then_with(|| left.qualified_name.cmp(&right.qualified_name))
    });
    maps
}

fn key_object_candidates(modules: &[MoveModule]) -> Vec<ObjectCandidate<'_>> {
    modules
        .iter()
        .filter(|module| !is_test_module(module))
        .flat_map(|module| {
            module
                .structs
                .iter()
                .filter(|move_struct| {
                    !has_test_attribute(&move_struct.attributes)
                        && struct_has_ability(move_struct, "key")
                })
                .map(|move_struct| ObjectCandidate {
                    module,
                    move_struct,
                    qualified_name: format!("{}::{}", module.name, move_struct.name),
                })
        })
        .collect()
}

fn function_index(modules: &[MoveModule]) -> BTreeMap<String, FunctionLookup<'_>> {
    modules
        .iter()
        .filter(|module| !is_test_module(module))
        .flat_map(|module| {
            module.functions.iter().filter_map(move |function| {
                if has_test_attribute(&function.attributes) {
                    return None;
                }

                Some((
                    format!("{}::{}", module.name, function.name),
                    FunctionLookup { module, function },
                ))
            })
        })
        .collect()
}

fn reverse_call_graph(
    functions: &BTreeMap<String, FunctionLookup<'_>>,
) -> BTreeMap<String, BTreeSet<String>> {
    let mut reverse = BTreeMap::<String, BTreeSet<String>>::new();

    for (caller_key, caller) in functions {
        let Some(body) = caller.function.body.as_deref() else {
            continue;
        };

        for (callee_key, callee) in functions {
            if caller_key == callee_key {
                continue;
            }

            if body_calls_function(
                body,
                &caller.module.name,
                &callee.module.name,
                &callee.function.name,
            ) {
                reverse
                    .entry(callee_key.clone())
                    .or_default()
                    .insert(caller_key.clone());
            }
        }
    }

    reverse
}

fn direct_lifecycle_events(
    objects: &[ObjectCandidate<'_>],
    functions: &BTreeMap<String, FunctionLookup<'_>>,
) -> Vec<DirectEvent> {
    let mut events = Vec::new();
    let mut seen = BTreeSet::new();

    for object in objects {
        for (function_key, lookup) in functions {
            for (stage, evidence) in
                function_lifecycle_events(object, lookup.module, lookup.function)
            {
                let key = format!(
                    "{}::{stage}::{function_key}::{evidence}",
                    object.qualified_name
                );

                if !seen.insert(key) {
                    continue;
                }

                events.push(DirectEvent {
                    object_key: object.qualified_name.clone(),
                    stage,
                    function_key: function_key.clone(),
                    evidence,
                });
            }
        }
    }

    events
}

fn function_lifecycle_events(
    object: &ObjectCandidate<'_>,
    module: &MoveModule,
    function: &MoveFunctionSignature,
) -> Vec<(String, String)> {
    let mut events = Vec::new();
    let Some(body) = function.body.as_deref() else {
        return events;
    };

    let type_name = &object.move_struct.name;
    let qualified_name = &object.qualified_name;
    let lower_body = body.to_ascii_lowercase();
    let constructs_type =
        body_constructs_type(body, type_name) || body_constructs_type(body, qualified_name);
    let returns_type = function_returns_type(&function.signature, type_name)
        || function_returns_type(&function.signature, qualified_name);
    let function_label = format!("{}::{}", module.name, function.name);

    if constructs_type && (lower_body.contains("object::new") || returns_type) {
        events.push((
            "created".to_string(),
            format!("{type_name} constructed in {function_label}"),
        ));
    }

    if function_mutably_touches_type(&function.signature, type_name)
        || function_mutably_touches_type(&function.signature, qualified_name)
    {
        events.push((
            "mutated".to_string(),
            format!("{function_label} takes &mut {type_name}"),
        ));
    }

    if borrowed_identity_mutates_related_state(body, &function.signature, type_name, qualified_name)
    {
        events.push((
            "mutated".to_string(),
            format!("{function_label} mutates state keyed by {type_name} identity"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::transfer",
            "transfer::public_transfer",
            "public_transfer",
        ],
    ) {
        events.push((
            "transferred".to_string(),
            format!("ownership transferred in {function_label}"),
        ));
        events.push((
            "owned".to_string(),
            format!("address-owned object path in {function_label}"),
        ));
    }

    if function.is_transaction_callable && returns_type {
        events.push((
            "transferred".to_string(),
            format!("returned to transaction caller from {function_label}"),
        ));
        events.push((
            "owned".to_string(),
            format!("returned from transaction-callable {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::share_object",
            "transfer::public_share_object",
            "share_object",
        ],
    ) {
        events.push((
            "shared".to_string(),
            format!("shared via transfer::share_object in {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::freeze_object",
            "transfer::public_freeze_object",
            "freeze_object",
        ],
    ) {
        events.push((
            "immutable".to_string(),
            format!("frozen via transfer::freeze_object in {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::party_transfer",
            "transfer::public_party_transfer",
            "party_transfer",
            "party::",
        ],
    ) {
        events.push((
            "party".to_string(),
            format!("moved through party ownership API in {function_label}"),
        ));
    }

    if delete_touches_type(body, &function.signature, type_name, qualified_name) {
        events.push((
            "deleted".to_string(),
            format!("delete signal observed in {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "dynamic_field::add",
            "dynamic_object_field::add",
            "table::add",
            "bag::add",
        ],
    ) {
        events.push((
            "wrapped".to_string(),
            format!("stored through dynamic object storage in {function_label}"),
        ));
    }

    events
}

fn wrapper_evidence(objects: &[ObjectCandidate<'_>]) -> BTreeMap<String, Vec<(String, String)>> {
    let mut evidence = BTreeMap::<String, Vec<(String, String)>>::new();

    for object in objects {
        for wrapper in objects {
            if object.qualified_name == wrapper.qualified_name {
                continue;
            }

            for field in &wrapper.move_struct.fields {
                if type_reference_matches(&field.type_name, &object.move_struct.name)
                    || type_reference_matches(&field.type_name, &object.qualified_name)
                {
                    evidence
                        .entry(object.qualified_name.clone())
                        .or_default()
                        .push((
                            "wrapped".to_string(),
                            format!(
                                "stored in {}::{}.{}",
                                wrapper.module.name, wrapper.move_struct.name, field.name
                            ),
                        ));
                }
            }
        }
    }

    evidence
}

struct IndirectCaller {
    caller_key: String,
    call_path: Vec<String>,
}

fn indirect_callers(
    direct_key: &str,
    reverse_call_graph: &BTreeMap<String, BTreeSet<String>>,
) -> Vec<IndirectCaller> {
    let mut callers = Vec::new();
    let mut queue = VecDeque::from([(
        direct_key.to_string(),
        vec![direct_key.to_string()],
        0_usize,
    )]);
    let mut visited = BTreeSet::new();

    while let Some((target, path_to_direct, depth)) = queue.pop_front() {
        if depth >= CALL_GRAPH_DEPTH {
            continue;
        }

        let Some(next_callers) = reverse_call_graph.get(&target) else {
            continue;
        };

        for caller_key in next_callers {
            if caller_key == direct_key || !visited.insert(caller_key.clone()) {
                continue;
            }

            let mut call_path = vec![caller_key.clone()];
            call_path.extend(path_to_direct.clone());
            callers.push(IndirectCaller {
                caller_key: caller_key.clone(),
                call_path: call_path.clone(),
            });
            queue.push_back((caller_key.clone(), call_path, depth + 1));
        }
    }

    callers
}

fn lifecycle_function_ref(
    lookup: FunctionLookup<'_>,
    direct: bool,
    call_path: Vec<String>,
    evidence: Vec<String>,
) -> ObjectLifecycleFunctionRef {
    ObjectLifecycleFunctionRef {
        module_name: lookup.module.name.clone(),
        function_name: lookup.function.name.clone(),
        qualified_name: format!("{}::{}", lookup.module.name, lookup.function.name),
        file_path: lookup.module.file_path.clone(),
        visibility: lookup.function.visibility.clone(),
        is_entry: lookup.function.is_entry,
        is_transaction_callable: lookup.function.is_transaction_callable,
        direct,
        call_path,
        evidence,
    }
}

fn push_stage_function(
    stage_functions: &mut BTreeMap<String, Vec<ObjectLifecycleFunctionRef>>,
    stage: &str,
    function_ref: ObjectLifecycleFunctionRef,
) {
    let functions = stage_functions.entry(stage.to_string()).or_default();

    if let Some(existing) = functions.iter_mut().find(|existing| {
        existing.qualified_name == function_ref.qualified_name
            && existing.direct == function_ref.direct
            && existing.call_path == function_ref.call_path
    }) {
        for evidence in function_ref.evidence {
            if !existing.evidence.contains(&evidence) {
                existing.evidence.push(evidence);
            }
        }
        return;
    }

    functions.push(function_ref);
}

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

fn sort_function_refs(functions: &mut Vec<ObjectLifecycleFunctionRef>) {
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

fn is_test_module(module: &MoveModule) -> bool {
    module
        .file_path
        .split('/')
        .any(|segment| segment == "tests")
        || has_test_attribute(&module.attributes)
}

fn has_test_attribute(attributes: &[String]) -> bool {
    attributes.iter().any(|attribute| {
        let attribute = attribute.to_ascii_lowercase();
        attribute.contains("test")
            || attribute.contains("test_only")
            || attribute.contains("random_test")
            || attribute.contains("expected_failure")
    })
}

fn struct_has_ability(move_struct: &MoveStructSignature, ability: &str) -> bool {
    move_struct
        .abilities
        .iter()
        .any(|candidate| candidate == ability)
}

fn function_mutably_touches_type(signature: &str, type_name: &str) -> bool {
    let Some(parameters) = function_parameters(signature) else {
        return false;
    };

    split_top_level(parameters, ',')
        .into_iter()
        .any(|parameter| {
            let Some((_, parameter_type)) = parameter.split_once(':') else {
                return false;
            };
            let parameter_type = parameter_type.trim();

            parameter_type.starts_with("&mut") && type_reference_matches(parameter_type, type_name)
        })
}

fn borrowed_identity_mutates_related_state(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
) -> bool {
    let body_block = function_body_block(body);
    let borrowed_names = borrowed_parameter_names(signature, type_name)
        .into_iter()
        .chain(borrowed_parameter_names(signature, qualified_name))
        .collect::<BTreeSet<_>>();

    if borrowed_names.is_empty() {
        return false;
    }

    let identity_names = object_identity_names(body_block, &borrowed_names);

    if identity_names.is_empty() {
        return false;
    }

    body_block.split(';').any(|statement| {
        identity_names
            .iter()
            .any(|identity| source_contains_identifier(statement, identity))
            && statement_has_mutation_signal(statement)
    })
}

fn function_body_block(function_source: &str) -> &str {
    let Some(start) = function_source.find('{') else {
        return function_source;
    };
    let Some(end) = function_source.rfind('}') else {
        return &function_source[start + 1..];
    };

    if start < end {
        &function_source[start + 1..end]
    } else {
        function_source
    }
}

fn borrowed_parameter_names(signature: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(parameters) = function_parameters(signature) else {
        return names;
    };

    for parameter in split_top_level(parameters, ',') {
        let Some((name, parameter_type)) = parameter.split_once(':') else {
            continue;
        };
        let parameter_type = parameter_type.trim();

        if !parameter_type.starts_with('&')
            || parameter_type.starts_with("&mut")
            || !type_reference_matches(parameter_type, type_name)
        {
            continue;
        }

        let name = name
            .split_whitespace()
            .last()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn object_identity_names(body: &str, object_names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in body.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };

        if !left.contains("let") {
            continue;
        }

        let derives_identity = object_names.iter().any(|object_name| {
            right.contains(&format!("object::id({object_name})"))
                || right.contains(&format!("object::id(&{object_name})"))
                || right.contains(&format!("{object_name}.id"))
        });

        if !derives_identity {
            continue;
        }

        let Some(raw_name) = left
            .split("let")
            .last()
            .and_then(|binding| binding.split(':').next())
        else {
            continue;
        };

        let name = raw_name
            .split_whitespace()
            .filter(|part| *part != "mut")
            .next_back()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn statement_has_mutation_signal(statement: &str) -> bool {
    let statement = statement.to_ascii_lowercase();
    const MUTATION_SIGNALS: &[&str] = &[
        "&mut",
        "_mut",
        "borrow_mut",
        "set_",
        "add_",
        "remove",
        "delete",
        "destroy",
        "insert",
        "push_back",
        ".add(",
        "::add(",
        ".remove(",
        "::remove(",
        "deposit",
        "withdraw",
        "supply",
        "split(",
        "decrease",
        "increase",
        "increment",
        "decrement",
        "latch",
        "refresh",
    ];

    MUTATION_SIGNALS
        .iter()
        .any(|signal| statement.contains(signal))
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

fn type_reference_matches(source: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    source
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '&' | ',' | ':' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ';' | '='
                )
        })
        .any(|token| token == short_name || token == type_name)
}

fn body_constructs_type(body: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    body.contains(&format!("{short_name} {{"))
        || body.contains(&format!("{short_name}<"))
        || body.contains(&format!("{type_name} {{"))
        || body.contains(&format!("{type_name}<"))
}

fn operation_touches_type(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
    operations: &[&str],
) -> bool {
    let value_names = owned_or_constructed_value_names(body, signature, type_name, qualified_name);

    operation_call_snippets(body, operations)
        .iter()
        .any(|snippet| {
            body_constructs_type(snippet, type_name)
                || body_constructs_type(snippet, qualified_name)
                || value_names
                    .iter()
                    .any(|value_name| source_contains_identifier(snippet, value_name))
        })
}

fn delete_touches_type(body: &str, signature: &str, type_name: &str, qualified_name: &str) -> bool {
    let lower_body = body.to_ascii_lowercase();

    if !is_delete_signal(&lower_body) {
        return false;
    }

    let value_names = owned_or_constructed_value_names(body, signature, type_name, qualified_name);
    let destructures_type =
        body_destructures_type(body, type_name) || body_destructures_type(body, qualified_name);

    destructures_type
        || value_names
            .iter()
            .any(|value_name| source_contains_identifier(body, value_name))
}

fn owned_or_constructed_value_names(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    names.extend(owned_parameter_names(signature, type_name));
    names.extend(owned_parameter_names(signature, qualified_name));
    names.extend(constructed_value_names(body, type_name));
    names.extend(constructed_value_names(body, qualified_name));
    names
}

fn owned_parameter_names(signature: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(parameters) = function_parameters(signature) else {
        return names;
    };

    for parameter in split_top_level(parameters, ',') {
        let Some((name, parameter_type)) = parameter.split_once(':') else {
            continue;
        };
        let parameter_type = parameter_type.trim();

        if parameter_type.starts_with('&') || !type_reference_matches(parameter_type, type_name) {
            continue;
        }

        let name = name
            .split_whitespace()
            .last()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn constructed_value_names(body: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in body.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };

        if !left.contains("let") || !body_constructs_type(right, type_name) {
            continue;
        }

        let Some(raw_name) = left
            .split("let")
            .last()
            .and_then(|binding| binding.split(':').next())
        else {
            continue;
        };

        let name = raw_name
            .split_whitespace()
            .filter(|part| *part != "mut")
            .next_back()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn body_destructures_type(body: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    body.contains(&format!("let {short_name} {{"))
        || body.contains(&format!("let {short_name}<"))
        || body.contains(&format!("let {type_name} {{"))
        || body.contains(&format!("let {type_name}<"))
}

fn operation_call_snippets(body: &str, operations: &[&str]) -> Vec<String> {
    let lower_body = body.to_ascii_lowercase();
    let mut snippets = Vec::new();

    for operation in operations {
        let operation = operation.to_ascii_lowercase();
        let mut search_start = 0;

        while let Some(relative_start) = lower_body[search_start..].find(&operation) {
            let start = search_start + relative_start;
            let end = lower_body[start..]
                .find(';')
                .map(|offset| start + offset)
                .unwrap_or(body.len());

            snippets.push(body[start..end].to_string());
            search_start = end.saturating_add(1);
        }
    }

    snippets
}

fn split_top_level(source: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0_i32;
    let mut paren_depth = 0_i32;

    for (index, character) in source.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            _ if character == delimiter && angle_depth == 0 && paren_depth == 0 => {
                parts.push(source[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(source[start..].trim());
    parts
}

fn source_contains_identifier(source: &str, identifier: &str) -> bool {
    source
        .split(|character: char| !is_identifier_character(character))
        .any(|token| token == identifier)
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn body_calls_function(
    body: &str,
    caller_module: &str,
    callee_module: &str,
    callee_name: &str,
) -> bool {
    let body = function_body_block(body);
    let qualified = format!("{callee_module}::{callee_name}");
    let qualified_call =
        body.contains(&format!("{qualified}(")) || body.contains(&format!("{qualified}<"));
    let same_module_call = caller_module == callee_module
        && (body.contains(&format!("{callee_name}(")) || body.contains(&format!("{callee_name}<")));

    qualified_call || same_module_call
}

fn is_delete_signal(lower_body: &str) -> bool {
    lower_body.contains(".delete(")
        || lower_body.contains("object::delete")
        || lower_body.contains("id.delete")
        || lower_body.contains("uid.delete")
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
        || name.contains("treasury")
}

fn receipt_or_position_like(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    ["receipt", "position", "ticket", "order", "session"]
        .iter()
        .any(|term| name.contains(term))
}
