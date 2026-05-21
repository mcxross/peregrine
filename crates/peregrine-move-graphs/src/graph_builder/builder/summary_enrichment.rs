impl GraphBuilder {
    fn enrich_from_summaries(&mut self, root: &Path, packages: &[MovePackageModel]) {
        let Some(summary_location) = resolve_summary_location(root, packages) else {
            return;
        };

        self.address_mapping =
            read_address_mapping(&summary_location.path.join("address_mapping.json"));
        let address_mapping = self.address_mapping.clone();

        for node in self.type_nodes.values_mut() {
            if node.canonical_address.is_none() {
                node.canonical_address =
                    canonical_address(&address_mapping, node.address.as_deref());
            }
        }

        let target_summary_ids = self.target_summary_ids(packages, &address_mapping);
        let summaries = read_summary_modules(&summary_location.path, &target_summary_ids);

        for summary in summaries {
            let module_address = summary.id.address.clone();
            let module_name = summary.id.name.clone();

            for (name, summary_struct) in &summary.structs {
                let source_id = self
                    .resolve_exact_type(Some(&module_address), &module_name, name)
                    .unwrap_or_else(|| {
                        self.ensure_external_type_node(
                            "struct",
                            Some(&module_address),
                            &module_name,
                            name,
                            "summary",
                        )
                    });

                for (field_name, field_type) in summary_fields(summary_struct) {
                    self.record_summary_type_value(
                        &source_id,
                        &field_type,
                        SummaryTypeUsageInput {
                            relationship: "field".to_string(),
                            field_name: Some(field_name.clone()),
                            declaring_type_id: Some(source_id.clone()),
                            declaring_field_name: Some(field_name),
                            span: summary_span(),
                            ..SummaryTypeUsageInput::default()
                        },
                    );
                }
            }

            for (name, summary_enum) in &summary.enums {
                let source_id = self
                    .resolve_exact_type(Some(&module_address), &module_name, name)
                    .unwrap_or_else(|| {
                        self.ensure_external_type_node(
                            "enum",
                            Some(&module_address),
                            &module_name,
                            name,
                            "summary",
                        )
                    });

                if let Some(variants) = summary_enum.get("variants").and_then(Value::as_object) {
                    for (variant_name, variant) in variants {
                        for (field_name, field_type) in summary_fields(variant) {
                            self.record_summary_type_value(
                                &source_id,
                                &field_type,
                                SummaryTypeUsageInput {
                                    relationship: "variantField".to_string(),
                                    field_name: Some(field_name.clone()),
                                    variant_name: Some(variant_name.clone()),
                                    declaring_type_id: Some(source_id.clone()),
                                    declaring_field_name: Some(field_name),
                                    span: summary_span(),
                                    ..SummaryTypeUsageInput::default()
                                },
                            );
                        }
                    }
                }
            }

            for (name, summary_function) in &summary.functions {
                let source_id = self
                    .resolve_exact_function(Some(&module_address), &module_name, name)
                    .unwrap_or_else(|| {
                        let member = MemberRef {
                            address: Some(module_address.clone()),
                            module: module_name.clone(),
                            member: name.clone(),
                        };
                        self.ensure_external_call_node(
                            &member,
                            &qualified_member(Some(&module_address), &module_name, name),
                            "summary",
                        )
                    });

                if let Some(parameters) =
                    summary_function.get("parameters").and_then(Value::as_array)
                {
                    for parameter in parameters {
                        let parameter_name = parameter
                            .get("name")
                            .and_then(Value::as_str)
                            .map(str::to_string);

                        if let Some(type_value) = parameter.get("type_") {
                            self.record_summary_type_value(
                                &source_id,
                                type_value,
                                SummaryTypeUsageInput {
                                    relationship: "parameter".to_string(),
                                    parameter_name,
                                    function_name: Some(name.clone()),
                                    span: summary_span(),
                                    ..SummaryTypeUsageInput::default()
                                },
                            );
                        }
                    }
                }

                if let Some(returns) = summary_function.get("return_").and_then(Value::as_array) {
                    for return_type in returns {
                        self.record_summary_type_value(
                            &source_id,
                            return_type,
                            SummaryTypeUsageInput {
                                relationship: "return".to_string(),
                                function_name: Some(name.clone()),
                                span: summary_span(),
                                ..SummaryTypeUsageInput::default()
                            },
                        );
                    }
                }
            }
        }
    }

    fn target_summary_ids(
        &self,
        packages: &[MovePackageModel],
        address_mapping: &BTreeMap<String, String>,
    ) -> BTreeSet<String> {
        let mut ids = packages
            .iter()
            .map(|package| package.name.clone())
            .collect::<BTreeSet<_>>();

        for node in self.type_nodes.values().filter(|node| !node.is_external) {
            if let Some(address) = &node.address {
                ids.insert(address.clone());
            }
            if let Some(canonical_address) = &node.canonical_address {
                ids.insert(canonical_address.clone());
            }
        }

        for value in ids.clone() {
            if let Some(mapped) = address_mapping.get(&value) {
                ids.insert(mapped.clone());
            }
        }

        ids
    }

    fn record_summary_type_value(
        &mut self,
        owner_id: &str,
        value: &Value,
        input: SummaryTypeUsageInput,
    ) -> Vec<TypeUse> {
        let mut uses = Vec::new();

        match value {
            Value::String(name) => {
                let id = if BUILTIN_TYPES.contains(&name.as_str()) {
                    builtin_type_id(name)
                } else {
                    self.ensure_external_type_node("summaryType", None, "summary", name, "summary")
                };
                self.record_type_edge(TypeEdgeInput {
                    source: owner_id.to_string(),
                    target: id.clone(),
                    relationship: input.relationship,
                    field_name: input.field_name,
                    variant_name: input.variant_name,
                    function_name: input.function_name,
                    parameter_name: input.parameter_name,
                    type_argument_index: input.type_argument_index,
                    is_mutable: input.is_mutable,
                    is_reference: input.is_reference,
                    type_expression: input.type_expression.or_else(|| Some(name.clone())),
                    declaring_type_id: input.declaring_type_id,
                    declaring_field_name: input.declaring_field_name,
                    type_argument_name: input.type_argument_name,
                    span: input.span,
                    confidence: "heuristic".to_string(),
                    evidence: vec![format!(
                        "Package summary records `{}` as `{name}`.",
                        owner_id
                    )],
                });
                uses.push(TypeUse { id });
            }
            Value::Object(object) => {
                if let Some(datatype) = object.get("Datatype").and_then(Value::as_object) {
                    if let (Some(module), Some(name)) = (
                        datatype.get("module").and_then(Value::as_object),
                        datatype.get("name").and_then(Value::as_str),
                    ) {
                        let address = module.get("address").and_then(Value::as_str);
                        let module_name = module
                            .get("name")
                            .and_then(Value::as_str)
                            .unwrap_or("unknown");
                        let id = self
                            .resolve_exact_type(address, module_name, name)
                            .unwrap_or_else(|| {
                                self.ensure_external_type_node(
                                    "datatype",
                                    address,
                                    module_name,
                                    name,
                                    "summary",
                                )
                            });

                        self.record_type_edge(TypeEdgeInput {
                            source: owner_id.to_string(),
                            target: id.clone(),
                            relationship: input.relationship.clone(),
                            field_name: input.field_name.clone(),
                            variant_name: input.variant_name.clone(),
                            function_name: input.function_name.clone(),
                            parameter_name: input.parameter_name.clone(),
                            type_argument_index: input.type_argument_index,
                            is_mutable: input.is_mutable,
                            is_reference: input.is_reference,
                            type_expression: input
                                .type_expression
                                .clone()
                                .or_else(|| Some(qualified_member(address, module_name, name))),
                            declaring_type_id: input.declaring_type_id.clone(),
                            declaring_field_name: input.declaring_field_name.clone(),
                            type_argument_name: input.type_argument_name.clone(),
                            span: input.span.clone(),
                            confidence: "heuristic".to_string(),
                            evidence: vec![format!(
                                "Package summary records `{}` as `{}`.",
                                owner_id,
                                qualified_member(address, module_name, name)
                            )],
                        });
                        uses.push(TypeUse { id: id.clone() });

                        if let Some(type_arguments) =
                            datatype.get("type_arguments").and_then(Value::as_array)
                        {
                            for (index, type_argument) in type_arguments.iter().enumerate() {
                                if let Some(argument) = type_argument.get("argument") {
                                    for argument_use in self.record_summary_type_value(
                                        &id,
                                        argument,
                                        SummaryTypeUsageInput {
                                            relationship: "genericArgument".to_string(),
                                            type_argument_index: Some(index),
                                            type_argument_name: self
                                                .generic_argument_name_for(&id, index),
                                            declaring_type_id: input.declaring_type_id.clone(),
                                            declaring_field_name: input
                                                .declaring_field_name
                                                .clone(),
                                            span: input.span.clone(),
                                            ..SummaryTypeUsageInput::default()
                                        },
                                    ) {
                                        uses.push(argument_use);
                                    }
                                }
                            }
                        }
                    }
                } else if let Some(inner) = object.get("vector") {
                    let vector_id = builtin_type_id("vector");
                    self.record_type_edge(TypeEdgeInput {
                        source: owner_id.to_string(),
                        target: vector_id.clone(),
                        relationship: input.relationship.clone(),
                        field_name: input.field_name.clone(),
                        variant_name: input.variant_name.clone(),
                        function_name: input.function_name.clone(),
                        parameter_name: input.parameter_name.clone(),
                        type_argument_index: input.type_argument_index,
                        is_mutable: input.is_mutable,
                        is_reference: input.is_reference,
                        type_expression: input
                            .type_expression
                            .clone()
                            .or_else(|| Some("vector".to_string())),
                        declaring_type_id: input.declaring_type_id.clone(),
                        declaring_field_name: input.declaring_field_name.clone(),
                        type_argument_name: input.type_argument_name.clone(),
                        span: input.span.clone(),
                        confidence: "heuristic".to_string(),
                        evidence: vec![format!(
                            "Package summary records `{owner_id}` as a vector storage type."
                        )],
                    });
                    uses.push(TypeUse {
                        id: vector_id.clone(),
                    });
                    uses.extend(self.record_summary_type_value(
                        &vector_id,
                        inner,
                        SummaryTypeUsageInput {
                            relationship: "genericArgument".to_string(),
                            type_argument_name: Some("T".to_string()),
                            declaring_type_id: input.declaring_type_id.clone(),
                            declaring_field_name: input.declaring_field_name.clone(),
                            span: input.span,
                            ..SummaryTypeUsageInput::default()
                        },
                    ));
                } else if let Some(reference) = object.get("Reference").and_then(Value::as_array) {
                    if let Some(inner) = reference.get(1) {
                        uses.extend(self.record_summary_type_value(
                            owner_id,
                            inner,
                            SummaryTypeUsageInput {
                                is_mutable:
                                    reference.first().and_then(Value::as_bool).unwrap_or(false),
                                is_reference: true,
                                ..input
                            },
                        ));
                    }
                } else {
                    for nested in object.values() {
                        uses.extend(self.record_summary_type_value(
                            owner_id,
                            nested,
                            input.clone(),
                        ));
                    }
                }
            }
            Value::Array(values) => {
                for value in values {
                    uses.extend(self.record_summary_type_value(owner_id, value, input.clone()));
                }
            }
            _ => {}
        }

        uses
    }

}
