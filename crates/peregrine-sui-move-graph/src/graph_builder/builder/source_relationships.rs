impl GraphBuilder {
    fn collect_source_type_relationships(&mut self, modules: &[SourceModule]) {
        for module in modules {
            let mut module_context = module_context(module);
            module_context.aliases = module_aliases(&module.module, &module_context.address);

            for member in &module.module.members {
                match member {
                    ModuleMember::Struct(move_struct) => {
                        self.collect_struct(&module_context, move_struct);
                    }
                    ModuleMember::Enum(move_enum) => {
                        self.collect_enum(&module_context, move_enum);
                    }
                    _ => {}
                }
            }
        }
    }

    fn collect_reachable_function_state_relationships(
        &mut self,
        modules: &[SourceModule],
        target: &MoveStateAccessGraphTarget,
    ) {
        let function_index = self.source_functions_by_id(modules);
        let Some(start_id) = self.resolve_target_function_id(target) else {
            return;
        };
        let mut visited = BTreeSet::new();
        let mut queue = VecDeque::from([(start_id, 0usize)]);

        while let Some((function_id, depth)) = queue.pop_front() {
            if !visited.insert(function_id.clone()) {
                continue;
            }

            let Some((module, function)) = function_index.get(&function_id).cloned() else {
                continue;
            };
            self.collect_function(&module, &function);

            if depth >= target.max_call_depth {
                continue;
            }

            let callees = self
                .call_edges
                .values()
                .filter(|edge| edge.source == function_id && edge.is_resolved && !edge.is_external)
                .map(|edge| edge.target.clone())
                .collect::<Vec<_>>();

            for callee in callees {
                if !visited.contains(&callee) && function_index.contains_key(&callee) {
                    queue.push_back((callee, depth + 1));
                }
            }
        }
    }

    fn source_functions_by_id(
        &self,
        modules: &[SourceModule],
    ) -> BTreeMap<String, (ModuleContext, Function)> {
        let mut functions = BTreeMap::new();

        for module in modules {
            let mut module_context = module_context(module);
            module_context.aliases = module_aliases(&module.module, &module_context.address);

            for member in &module.module.members {
                let ModuleMember::Function(function) = member else {
                    continue;
                };
                let function_name = function.name.0.value.to_string();
                let Some(function_id) = self.resolve_exact_function(
                    module_context.address.as_deref(),
                    &module_context.module_name,
                    &function_name,
                ) else {
                    continue;
                };

                functions.insert(function_id, (module_context.clone(), function.clone()));
            }
        }

        functions
    }

    fn resolve_target_function_id(&self, target: &MoveStateAccessGraphTarget) -> Option<String> {
        self.call_nodes
            .values()
            .find(|node| {
                node.package_path.as_deref() == Some(target.package_path.as_str())
                    && node.address.as_deref() == target.address.as_deref()
                    && node.module_name == target.module_name
                    && node.function_name == target.function_name
            })
            .map(|node| node.id.clone())
            .or_else(|| {
                self.resolve_exact_function(
                    target.address.as_deref(),
                    &target.module_name,
                    &target.function_name,
                )
            })
    }

    fn collect_function(&mut self, module: &ModuleContext, function: &Function) {
        let function_name = function.name.0.value.to_string();
        let Some(function_id) = self.resolve_exact_function(
            module.address.as_deref(),
            &module.module_name,
            &function_name,
        ) else {
            return;
        };
        let mut type_parameters = BTreeMap::new();

        for (type_parameter, constraints) in &function.signature.type_parameters {
            let id = type_parameter_id(&function_id, type_parameter.value.as_ref());

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id: id.clone(),
                    kind: "typeParameter".to_string(),
                    package_name: Some(module.package_name.clone()),
                    package_path: Some(module.package_path.clone()),
                    address: module.address.clone(),
                    canonical_address: canonical_address(
                        &self.address_mapping,
                        module.address.as_deref(),
                    ),
                    module_name: Some(module.module_name.clone()),
                    name: type_parameter.value.to_string(),
                    qualified_name: format!("{function_id}::{}", type_parameter.value),
                    file_path: Some(module.file_path.clone()),
                    abilities: constraints
                        .iter()
                        .map(|ability| ability_name(&ability.value).to_string())
                        .collect(),
                    type_parameters: Vec::new(),
                    attributes: Vec::new(),
                    span: Some(source_span(
                        &module.source,
                        &module.file_path,
                        type_parameter.loc.start() as usize,
                        type_parameter.loc.end() as usize,
                    )),
                    source: "source".to_string(),
                    is_external: false,
                });
            self.record_type_edge(TypeEdgeInput {
                source: function_id.clone(),
                target: id.clone(),
                relationship: "typeParameter".to_string(),
                span: source_span(
                    &module.source,
                    &module.file_path,
                    type_parameter.loc.start() as usize,
                    type_parameter.loc.end() as usize,
                ),
                ..TypeEdgeInput::default()
            });
            type_parameters.insert(type_parameter.value.to_string(), id);
        }

        let type_context = TypeContext {
            owner_id: function_id.clone(),
            owner_name: Some(function_name.clone()),
            module: module.clone(),
            type_parameters,
        };

        let mut local_state_types = BTreeMap::new();

        for (_, parameter, parameter_type) in &function.signature.parameters {
            let parameter_name = parameter.0.value.to_string();
            let parameter_uses = self.record_type_usage(
                &type_context,
                parameter_type,
                TypeUsageInput {
                    relationship: "parameter".to_string(),
                    parameter_name: Some(parameter_name.clone()),
                    function_name: Some(function_name.clone()),
                    ..TypeUsageInput::default()
                },
            );
            self.record_state_type_accesses(
                &function_id,
                parameter_uses.clone(),
                state_access_kind_for_parameter_type(parameter_type),
                Some(parameter_name.clone()),
                None,
                source_span(
                    &module.source,
                    &module.file_path,
                    parameter_type.loc.start() as usize,
                    parameter_type.loc.end() as usize,
                ),
                vec![format!(
                    "Function parameter `{parameter_name}` exposes package state to this call."
                )],
            );

            if let Some(state_type_id) = self.first_state_type_id(&parameter_uses) {
                local_state_types.insert(parameter_name, state_type_id);
            }
        }

        let return_uses = self.record_type_usage(
            &type_context,
            &function.signature.return_type,
            TypeUsageInput {
                relationship: "return".to_string(),
                function_name: Some(function_name.clone()),
                ..TypeUsageInput::default()
            },
        );
        self.record_state_type_accesses(
            &function_id,
            return_uses,
            "return",
            None,
            None,
            source_span(
                &module.source,
                &module.file_path,
                function.signature.return_type.loc.start() as usize,
                function.signature.return_type.loc.end() as usize,
            ),
            vec!["Function return type can move package state to the caller.".to_string()],
        );

        if let FunctionBody_::Defined(sequence) = &function.body.value {
            let mut function_context = FunctionContext {
                module: module.clone(),
                function_id,
                function_name,
                type_parameters: type_context.type_parameters.clone(),
                local_state_types,
            };
            self.traverse_sequence(&mut function_context, &module.aliases, sequence);
        }
    }

    fn collect_struct(&mut self, module: &ModuleContext, move_struct: &StructDefinition) {
        let name = move_struct.name.0.value.to_string();
        let Some(struct_id) =
            self.resolve_exact_type(module.address.as_deref(), &module.module_name, &name)
        else {
            return;
        };
        let mut type_parameters = BTreeMap::new();

        for type_parameter in &move_struct.type_parameters {
            let id = type_parameter_id(&struct_id, type_parameter.name.value.as_ref());

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id: id.clone(),
                    kind: "typeParameter".to_string(),
                    package_name: Some(module.package_name.clone()),
                    package_path: Some(module.package_path.clone()),
                    address: module.address.clone(),
                    canonical_address: canonical_address(
                        &self.address_mapping,
                        module.address.as_deref(),
                    ),
                    module_name: Some(module.module_name.clone()),
                    name: type_parameter.name.value.to_string(),
                    qualified_name: format!("{struct_id}::{}", type_parameter.name.value),
                    file_path: Some(module.file_path.clone()),
                    abilities: type_parameter
                        .constraints
                        .iter()
                        .map(|ability| ability_name(&ability.value).to_string())
                        .collect(),
                    type_parameters: Vec::new(),
                    attributes: Vec::new(),
                    span: Some(source_span(
                        &module.source,
                        &module.file_path,
                        type_parameter.name.loc.start() as usize,
                        type_parameter.name.loc.end() as usize,
                    )),
                    source: "source".to_string(),
                    is_external: false,
                });
            self.record_type_edge(TypeEdgeInput {
                source: struct_id.clone(),
                target: id.clone(),
                relationship: if type_parameter.is_phantom {
                    "phantomTypeParameter"
                } else {
                    "typeParameter"
                }
                .to_string(),
                span: source_span(
                    &module.source,
                    &module.file_path,
                    type_parameter.name.loc.start() as usize,
                    type_parameter.name.loc.end() as usize,
                ),
                ..TypeEdgeInput::default()
            });
            type_parameters.insert(type_parameter.name.value.to_string(), id);
        }

        let is_state_struct = self.is_state_type(&struct_id);
        let type_context = TypeContext {
            owner_id: struct_id.clone(),
            owner_name: Some(name),
            module: module.clone(),
            type_parameters,
        };

        match &move_struct.fields {
            StructFields::Named(fields) => {
                for (_, field, field_type) in fields {
                    let field_name = field.0.value.to_string();
                    self.record_type_usage(
                        &type_context,
                        field_type,
                        TypeUsageInput {
                            relationship: "field".to_string(),
                            field_name: Some(field_name.clone()),
                            ..TypeUsageInput::default()
                        },
                    );
                    if is_state_struct {
                        self.ensure_state_field_node(&struct_id, &field_name);
                    }
                }
            }
            StructFields::Positional(fields) => {
                for (index, (_, field_type)) in fields.iter().enumerate() {
                    let field_name = index.to_string();
                    self.record_type_usage(
                        &type_context,
                        field_type,
                        TypeUsageInput {
                            relationship: "field".to_string(),
                            field_name: Some(field_name.clone()),
                            ..TypeUsageInput::default()
                        },
                    );
                    if is_state_struct {
                        self.ensure_state_field_node(&struct_id, &field_name);
                    }
                }
            }
            StructFields::Native(_) => {}
        }
    }

    fn collect_enum(&mut self, module: &ModuleContext, move_enum: &EnumDefinition) {
        let name = move_enum.name.0.value.to_string();
        let Some(enum_id) =
            self.resolve_exact_type(module.address.as_deref(), &module.module_name, &name)
        else {
            return;
        };
        let mut type_parameters = BTreeMap::new();

        for type_parameter in &move_enum.type_parameters {
            let id = type_parameter_id(&enum_id, type_parameter.name.value.as_ref());

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id: id.clone(),
                    kind: "typeParameter".to_string(),
                    package_name: Some(module.package_name.clone()),
                    package_path: Some(module.package_path.clone()),
                    address: module.address.clone(),
                    canonical_address: canonical_address(
                        &self.address_mapping,
                        module.address.as_deref(),
                    ),
                    module_name: Some(module.module_name.clone()),
                    name: type_parameter.name.value.to_string(),
                    qualified_name: format!("{enum_id}::{}", type_parameter.name.value),
                    file_path: Some(module.file_path.clone()),
                    abilities: type_parameter
                        .constraints
                        .iter()
                        .map(|ability| ability_name(&ability.value).to_string())
                        .collect(),
                    type_parameters: Vec::new(),
                    attributes: Vec::new(),
                    span: Some(source_span(
                        &module.source,
                        &module.file_path,
                        type_parameter.name.loc.start() as usize,
                        type_parameter.name.loc.end() as usize,
                    )),
                    source: "source".to_string(),
                    is_external: false,
                });
            self.record_type_edge(TypeEdgeInput {
                source: enum_id.clone(),
                target: id.clone(),
                relationship: if type_parameter.is_phantom {
                    "phantomTypeParameter"
                } else {
                    "typeParameter"
                }
                .to_string(),
                span: source_span(
                    &module.source,
                    &module.file_path,
                    type_parameter.name.loc.start() as usize,
                    type_parameter.name.loc.end() as usize,
                ),
                ..TypeEdgeInput::default()
            });
            type_parameters.insert(type_parameter.name.value.to_string(), id);
        }

        let type_context = TypeContext {
            owner_id: enum_id,
            owner_name: Some(name),
            module: module.clone(),
            type_parameters,
        };

        for variant in &move_enum.variants {
            let variant_name = variant.name.0.value.to_string();

            match &variant.fields {
                VariantFields::Named(fields) => {
                    for (_, field, field_type) in fields {
                        self.record_type_usage(
                            &type_context,
                            field_type,
                            TypeUsageInput {
                                relationship: "variantField".to_string(),
                                field_name: Some(field.0.value.to_string()),
                                variant_name: Some(variant_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                VariantFields::Positional(fields) => {
                    for (index, (_, field_type)) in fields.iter().enumerate() {
                        self.record_type_usage(
                            &type_context,
                            field_type,
                            TypeUsageInput {
                                relationship: "variantField".to_string(),
                                field_name: Some(index.to_string()),
                                variant_name: Some(variant_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                VariantFields::Empty => {}
            }
        }
    }

}
