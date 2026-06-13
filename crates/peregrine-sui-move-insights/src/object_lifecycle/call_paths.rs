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
