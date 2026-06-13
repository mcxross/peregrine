impl GraphBuilder {
    fn record_call(
        &mut self,
        function: &mut FunctionContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
        exp: &Exp,
        arguments: &[Exp],
        default_kind: &str,
    ) {
        let parts = name_access_parts(name);
        let call_kind = if parts.iter().any(|part| part.is_macro) {
            "macro"
        } else {
            default_kind
        };
        let raw_target = name_access_to_string(name);
        let type_arguments = name_access_type_arguments(name, &function.module.source);
        let span = source_span(
            &function.module.source,
            &function.module.file_path,
            exp.loc.start() as usize,
            exp.loc.end() as usize,
        );

        for type_argument in name_access_types(name) {
            self.record_function_type_usage(
                function,
                type_argument,
                TypeUsageInput {
                    relationship: "callTypeArgument".to_string(),
                    function_name: Some(function.function_name.clone()),
                    ..TypeUsageInput::default()
                },
            );
        }

        match self.resolve_call_target(function, aliases, name) {
            ResolvedCall::Local(target_id) => {
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id.clone(),
                    call_kind: call_kind.to_string(),
                    confidence: "high".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments,
                    span: span.clone(),
                    is_external: false,
                    is_resolved: true,
                });
                self.record_state_edge(StateAccessEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id.clone(),
                    access_kind: "call".to_string(),
                    via_function: Some(target_id),
                    span,
                    confidence: "high".to_string(),
                    evidence: vec![format!(
                        "Function call `{raw_target}` can propagate state access."
                    )],
                    ..StateAccessEdgeInput::default()
                });
            }
            ResolvedCall::External(target) => {
                let target_id = self.ensure_external_call_node(&target, &raw_target, "source");
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "medium".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments,
                    span: span.clone(),
                    is_external: true,
                    is_resolved: false,
                });
                self.record_unresolved_call(
                    function,
                    raw_target,
                    call_kind,
                    span,
                    "target function is outside parsed source",
                );
            }
            ResolvedCall::Unresolved(reason) => {
                let target_id = self.ensure_unresolved_call_node(&raw_target);
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "low".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments,
                    span: span.clone(),
                    is_external: false,
                    is_resolved: false,
                });
                self.record_unresolved_call(function, raw_target, call_kind, span, reason);
            }
        }

        for argument in arguments {
            self.traverse_exp(function, aliases, argument);
        }
    }

    fn record_dot_call(
        &mut self,
        function: &mut FunctionContext,
        aliases: &AliasScope,
        method: &Name,
        is_macro: bool,
        type_arguments: &[Type],
        arguments: &[Exp],
        exp: &Exp,
    ) {
        let method_name = method.value.to_string();
        let raw_target = format!(".{method_name}");
        let call_kind = if is_macro { "methodMacro" } else { "method" };
        let span = source_span(
            &function.module.source,
            &function.module.file_path,
            exp.loc.start() as usize,
            exp.loc.end() as usize,
        );
        let type_argument_sources = type_arguments
            .iter()
            .map(|type_argument| ast_type_source(type_argument, &function.module.source))
            .collect::<Vec<_>>();

        for type_argument in type_arguments {
            self.record_function_type_usage(
                function,
                type_argument,
                TypeUsageInput {
                    relationship: "callTypeArgument".to_string(),
                    function_name: Some(function.function_name.clone()),
                    ..TypeUsageInput::default()
                },
            );
        }

        if let Some(target) = aliases.method_aliases.get(&method_name) {
            if let Some(target_id) = self.resolve_exact_function(
                target.address.as_deref(),
                &target.module,
                &target.member,
            ) {
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id.clone(),
                    call_kind: call_kind.to_string(),
                    confidence: "high".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments: type_argument_sources,
                    span: span.clone(),
                    is_external: false,
                    is_resolved: true,
                });
                self.record_state_edge(StateAccessEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id.clone(),
                    access_kind: "call".to_string(),
                    via_function: Some(target_id),
                    span,
                    confidence: "high".to_string(),
                    evidence: vec![format!(
                        "Method call `{raw_target}` can propagate state access."
                    )],
                    ..StateAccessEdgeInput::default()
                });
            } else {
                let target_id = self.ensure_external_call_node(target, &raw_target, "source");
                self.record_call_edge(CallEdgeInput {
                    source: function.function_id.clone(),
                    target: target_id,
                    call_kind: call_kind.to_string(),
                    confidence: "medium".to_string(),
                    raw_target: raw_target.clone(),
                    type_arguments: type_argument_sources,
                    span: span.clone(),
                    is_external: true,
                    is_resolved: false,
                });
                self.record_unresolved_call(
                    function,
                    raw_target,
                    call_kind,
                    span,
                    "use fun method target is outside parsed source",
                );
            }
        } else {
            let target_id = self.ensure_unresolved_call_node(&raw_target);
            self.record_call_edge(CallEdgeInput {
                source: function.function_id.clone(),
                target: target_id,
                call_kind: call_kind.to_string(),
                confidence: "low".to_string(),
                raw_target: raw_target.clone(),
                type_arguments: type_argument_sources,
                span: span.clone(),
                is_external: false,
                is_resolved: false,
            });
            self.record_unresolved_call(
                function,
                raw_target,
                call_kind,
                span,
                "method receiver type is not inferred by the parser graph pass",
            );
        }

        for argument in arguments {
            self.traverse_exp(function, aliases, argument);
        }
    }

    fn record_construct_or_destructure(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
        relationship: &str,
        span: MoveSourceSpan,
        function_name: Option<String>,
    ) -> Option<String> {
        let type_context = TypeContext {
            owner_id: function.function_id.clone(),
            owner_name: Some(function.function_name.clone()),
            module: function.module.clone(),
            type_parameters: function.type_parameters.clone(),
        };

        let type_use =
            self.resolve_type_apply(&type_context, aliases, name, span.clone(), relationship)?;
        let type_id = type_use.id.clone();
        self.record_type_edge(TypeEdgeInput {
            source: function.function_id.clone(),
            target: type_use.id,
            relationship: relationship.to_string(),
            function_name,
            span: span.clone(),
            confidence: "syntactic".to_string(),
            ..TypeEdgeInput::default()
        });
        self.record_state_type_accesses(
            &function.function_id,
            vec![TypeUse {
                id: type_id.clone(),
            }],
            if relationship == "construction" {
                "write"
            } else {
                "read"
            },
            None,
            None,
            span,
            vec![format!("AST {relationship} touches package state type.")],
        );
        Some(type_id)
    }

}
