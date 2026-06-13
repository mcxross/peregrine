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

pub fn object_lifecycle_risks(
    modules: &[MoveModule],
    lifecycle_map: &ObjectLifecycleMap,
) -> Vec<ObjectLifecycleRisk> {
    let functions = function_index(modules);

    lifecycle_risks(
        &lifecycle_map.qualified_name,
        &lifecycle_map.type_name,
        lifecycle_map.is_capability_like,
        &lifecycle_map.stages,
        &functions,
    )
}

