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

