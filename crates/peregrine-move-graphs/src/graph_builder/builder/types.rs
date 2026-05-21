impl GraphBuilder {
    fn record_function_type_usage(
        &mut self,
        function: &FunctionContext,
        type_: &Type,
        input: TypeUsageInput,
    ) -> Vec<TypeUse> {
        let type_context = TypeContext {
            owner_id: function.function_id.clone(),
            owner_name: Some(function.function_name.clone()),
            module: function.module.clone(),
            type_parameters: function.type_parameters.clone(),
        };

        self.record_type_usage(&type_context, type_, input)
    }

    fn record_type_usage(
        &mut self,
        context: &TypeContext,
        type_: &Type,
        input: TypeUsageInput,
    ) -> Vec<TypeUse> {
        match &type_.value {
            Type_::Apply(name) => {
                let span = source_span(
                    &context.module.source,
                    &context.module.file_path,
                    type_.loc.start() as usize,
                    type_.loc.end() as usize,
                );
                let type_expression = ast_type_source(type_, &context.module.source);
                let Some(type_use) = self.resolve_type_apply(
                    context,
                    &context.module.aliases,
                    name,
                    span.clone(),
                    &input.relationship,
                ) else {
                    return Vec::new();
                };

                self.record_type_edge(TypeEdgeInput {
                    source: context.owner_id.clone(),
                    target: type_use.id.clone(),
                    relationship: input.relationship.clone(),
                    field_name: input.field_name.clone(),
                    variant_name: input.variant_name.clone(),
                    function_name: input
                        .function_name
                        .clone()
                        .or_else(|| context.owner_name.clone()),
                    parameter_name: input.parameter_name.clone(),
                    type_argument_index: input.type_argument_index,
                    is_mutable: input.is_mutable,
                    is_reference: input.is_reference,
                    type_expression: Some(type_expression.clone()),
                    declaring_type_id: input
                        .declaring_type_id
                        .clone()
                        .or_else(|| Some(context.owner_id.clone())),
                    declaring_field_name: input
                        .declaring_field_name
                        .clone()
                        .or_else(|| input.field_name.clone()),
                    span: span.clone(),
                    confidence: "syntactic".to_string(),
                    evidence: type_usage_evidence(context, &type_use.id, &type_expression, &input),
                    ..TypeEdgeInput::default()
                });

                for (index, type_argument) in name_access_types(name).into_iter().enumerate() {
                    for argument in self.record_type_usage(
                        context,
                        type_argument,
                        TypeUsageInput {
                            relationship: "genericArgument".to_string(),
                            type_argument_index: Some(index),
                            field_name: input.field_name.clone(),
                            variant_name: input.variant_name.clone(),
                            function_name: input.function_name.clone(),
                            parameter_name: input.parameter_name.clone(),
                            declaring_type_id: input
                                .declaring_type_id
                                .clone()
                                .or_else(|| Some(context.owner_id.clone())),
                            declaring_field_name: input
                                .declaring_field_name
                                .clone()
                                .or_else(|| input.field_name.clone()),
                            ..TypeUsageInput::default()
                        },
                    ) {
                        let argument_expression =
                            ast_type_source(type_argument, &context.module.source);
                        let type_argument_name =
                            self.generic_argument_name_for(&type_use.id, index);
                        self.record_type_edge(TypeEdgeInput {
                            source: type_use.id.clone(),
                            target: argument.id,
                            relationship: "genericArgument".to_string(),
                            type_argument_index: Some(index),
                            type_expression: Some(argument_expression.clone()),
                            declaring_type_id: input
                                .declaring_type_id
                                .clone()
                                .or_else(|| Some(context.owner_id.clone())),
                            declaring_field_name: input
                                .declaring_field_name
                                .clone()
                                .or_else(|| input.field_name.clone()),
                            type_argument_name: type_argument_name.clone(),
                            span: source_span(
                                &context.module.source,
                                &context.module.file_path,
                                type_argument.loc.start() as usize,
                                type_argument.loc.end() as usize,
                            ),
                            confidence: "syntactic".to_string(),
                            evidence: generic_argument_evidence(
                                &type_use.id,
                                type_argument_name.as_deref(),
                                index,
                                &argument_expression,
                                &input,
                            ),
                            ..TypeEdgeInput::default()
                        });
                    }
                }

                vec![type_use]
            }
            Type_::Ref(is_mutable, inner) => self.record_type_usage(
                context,
                inner,
                TypeUsageInput {
                    is_mutable: *is_mutable || input.is_mutable,
                    is_reference: true,
                    ..input
                },
            ),
            Type_::Fun(parameters, return_type) => {
                let mut uses = Vec::new();

                for parameter in parameters {
                    uses.extend(self.record_type_usage(
                        context,
                        parameter,
                        TypeUsageInput {
                            relationship: input.relationship.clone(),
                            ..input.clone()
                        },
                    ));
                }
                uses.extend(self.record_type_usage(context, return_type, input));
                uses
            }
            Type_::Multiple(types) => types
                .iter()
                .flat_map(|type_| self.record_type_usage(context, type_, input.clone()))
                .collect(),
            Type_::Unit | Type_::UnresolvedError => Vec::new(),
        }
    }

}
