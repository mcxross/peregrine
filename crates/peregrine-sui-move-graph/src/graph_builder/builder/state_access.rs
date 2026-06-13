impl GraphBuilder {
    fn first_state_type_id(&self, uses: &[TypeUse]) -> Option<String> {
        uses.iter().find_map(|type_use| {
            self.is_state_type(&type_use.id)
                .then(|| type_use.id.clone())
        })
    }

    fn record_state_type_accesses(
        &mut self,
        function_id: &str,
        uses: Vec<TypeUse>,
        access_kind: &str,
        field_name: Option<String>,
        via_function: Option<String>,
        span: MoveSourceSpan,
        evidence: Vec<String>,
    ) {
        for type_use in uses {
            if !self.ensure_state_type_node(&type_use.id) {
                continue;
            }

            self.record_state_edge(StateAccessEdgeInput {
                source: function_id.to_string(),
                target: type_use.id,
                access_kind: access_kind.to_string(),
                field_name: field_name.clone(),
                via_function: via_function.clone(),
                span: span.clone(),
                confidence: "syntactic".to_string(),
                evidence: evidence.clone(),
            });
        }
    }

    fn record_state_field_access(
        &mut self,
        function: &FunctionContext,
        type_id: &str,
        field_name: &str,
        access_kind: &str,
        span: MoveSourceSpan,
        evidence: Vec<String>,
    ) {
        let Some(field_id) = self.ensure_state_field_node(type_id, field_name) else {
            return;
        };

        self.record_state_edge(StateAccessEdgeInput {
            source: function.function_id.clone(),
            target: field_id,
            access_kind: access_kind.to_string(),
            field_name: Some(field_name.to_string()),
            via_function: None,
            span,
            confidence: "syntactic".to_string(),
            evidence,
        });
    }

    fn record_field_access_from_receiver(
        &mut self,
        function: &FunctionContext,
        receiver: &Exp,
        field_name: &str,
        access_kind: &str,
        span: MoveSourceSpan,
    ) {
        if let Some(type_id) = self.local_state_type_from_exp(function, receiver) {
            self.record_state_field_access(
                function,
                &type_id,
                field_name,
                access_kind,
                span,
                vec![format!(
                    "Field `{field_name}` is accessed through a local value with package state type."
                )],
            );
        }
    }

    fn traverse_state_target_exp(
        &mut self,
        function: &mut FunctionContext,
        aliases: &AliasScope,
        exp: &Exp,
        access_kind: &str,
    ) {
        self.record_exp_state_access(function, exp, access_kind);

        match &exp.value {
            Exp_::Dot(receiver, _, _) => self.traverse_exp(function, aliases, receiver),
            Exp_::Name(_) => {}
            Exp_::Parens(inner) | Exp_::Dereference(inner) => {
                self.traverse_state_target_exp(function, aliases, inner, access_kind);
            }
            _ => self.traverse_exp(function, aliases, exp),
        }
    }

    fn record_exp_state_access(
        &mut self,
        function: &FunctionContext,
        exp: &Exp,
        access_kind: &str,
    ) {
        let span = source_span(
            &function.module.source,
            &function.module.file_path,
            exp.loc.start() as usize,
            exp.loc.end() as usize,
        );

        match &exp.value {
            Exp_::Name(name) => self.record_name_state_access(function, name, access_kind, span),
            Exp_::Dot(receiver, _, field) => self.record_field_access_from_receiver(
                function,
                receiver,
                &field.value.to_string(),
                access_kind,
                span,
            ),
            Exp_::Parens(inner) | Exp_::Dereference(inner) => {
                self.record_exp_state_access(function, inner, access_kind);
            }
            _ => {}
        }
    }

    fn record_name_state_access(
        &mut self,
        function: &FunctionContext,
        name: &NameAccessChain,
        access_kind: &str,
        span: MoveSourceSpan,
    ) {
        let Some(local_name) = name_access_local_name(name) else {
            return;
        };
        let Some(type_id) = function.local_state_types.get(&local_name).cloned() else {
            return;
        };

        self.record_state_type_accesses(
            &function.function_id,
            vec![TypeUse { id: type_id }],
            access_kind,
            None,
            None,
            span,
            vec![format!(
                "Local `{local_name}` has a package state type in this function."
            )],
        );
    }

    fn local_state_type_from_exp(&self, function: &FunctionContext, exp: &Exp) -> Option<String> {
        match &exp.value {
            Exp_::Name(name) => name_access_local_name(name)
                .and_then(|local_name| function.local_state_types.get(&local_name).cloned()),
            Exp_::Parens(inner) | Exp_::Dereference(inner) => {
                self.local_state_type_from_exp(function, inner)
            }
            _ => None,
        }
    }

    fn bind_first_state_type(
        &self,
        function: &mut FunctionContext,
        bindings: &[Bind],
        uses: &[TypeUse],
    ) {
        if let Some(state_type_id) = self.first_state_type_id(uses) {
            self.bind_state_type(function, bindings, &state_type_id);
        }
    }

    fn bind_state_type(
        &self,
        function: &mut FunctionContext,
        bindings: &[Bind],
        state_type_id: &str,
    ) {
        for binding in bindings {
            if let Bind_::Var(_, name) = &binding.value {
                function
                    .local_state_types
                    .insert(name.0.value.to_string(), state_type_id.to_string());
            }
        }
    }

    fn state_type_id_from_exp(
        &mut self,
        function: &FunctionContext,
        aliases: &AliasScope,
        exp: &Exp,
    ) -> Option<String> {
        let Exp_::Pack(name, _) = &exp.value else {
            return None;
        };
        let type_context = TypeContext {
            owner_id: function.function_id.clone(),
            owner_name: Some(function.function_name.clone()),
            module: function.module.clone(),
            type_parameters: function.type_parameters.clone(),
        };
        let type_use = self.resolve_type_apply(
            &type_context,
            aliases,
            name,
            source_span(
                &function.module.source,
                &function.module.file_path,
                exp.loc.start() as usize,
                exp.loc.end() as usize,
            ),
            "localBinding",
        )?;

        self.is_state_type(&type_use.id).then_some(type_use.id)
    }

    fn is_state_type(&self, type_id: &str) -> bool {
        self.type_nodes.get(type_id).is_some_and(is_state_type_node)
    }

    fn ensure_state_type_node(&mut self, type_id: &str) -> bool {
        if self.state_nodes.contains_key(type_id) {
            return true;
        }

        let Some(node) = self.type_nodes.get(type_id).cloned() else {
            return false;
        };
        if !is_state_type_node(&node) {
            return false;
        }

        self.state_nodes.insert(
            type_id.to_string(),
            state_node_from_type_node(&node, "stateType"),
        );
        true
    }

    fn ensure_state_field_node(&mut self, type_id: &str, field_name: &str) -> Option<String> {
        if !self.ensure_state_type_node(type_id) {
            return None;
        }

        let type_node = self.type_nodes.get(type_id)?.clone();
        let id = state_field_id(type_id, field_name);
        self.state_nodes
            .entry(id.clone())
            .or_insert_with(|| MoveStateAccessGraphNode {
                id: id.clone(),
                kind: "field".to_string(),
                package_name: type_node.package_name.clone(),
                package_path: type_node.package_path.clone(),
                address: type_node.address.clone(),
                module_name: type_node.module_name.clone(),
                name: field_name.to_string(),
                qualified_name: format!("{}.{}", type_node.qualified_name, field_name),
                file_path: type_node.file_path.clone(),
                abilities: Vec::new(),
                span: None,
                is_external: type_node.is_external,
                source: type_node.source.clone(),
            });
        Some(id)
    }

    fn record_state_edge(&mut self, input: StateAccessEdgeInput) {
        let key = StateAccessEdgeKey {
            source: input.source.clone(),
            target: input.target.clone(),
            access_kind: input.access_kind.clone(),
            field_name: input.field_name.clone(),
            via_function: input.via_function.clone(),
        };
        let edge = self
            .state_edges
            .entry(key)
            .or_insert_with(|| MoveStateAccessGraphEdge {
                source: input.source,
                target: input.target,
                access_kind: input.access_kind,
                field_name: input.field_name,
                via_function: input.via_function,
                source_spans: Vec::new(),
                confidence: input.confidence,
                evidence: Vec::new(),
            });

        edge.source_spans.push(input.span);
        edge.evidence.extend(input.evidence);
    }

}
