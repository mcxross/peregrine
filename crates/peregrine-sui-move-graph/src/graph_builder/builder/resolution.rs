impl GraphBuilder {
    fn resolve_call_target(
        &self,
        function: &FunctionContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
    ) -> ResolvedCall {
        let parts = name_access_parts(name);

        match parts.as_slice() {
            [single] => {
                if let Some(member) = aliases.member_aliases.get(&single.name) {
                    return self.resolve_member_call(member);
                }

                if let Some(target_id) = self.resolve_exact_function(
                    function.module.address.as_deref(),
                    &function.module.module_name,
                    &single.name,
                ) {
                    ResolvedCall::Local(target_id)
                } else {
                    ResolvedCall::Unresolved(
                        "single-name call did not match a local function or imported member",
                    )
                }
            }
            [module, member] => {
                if let Some(module_ref) = aliases.module_aliases.get(&module.name) {
                    let target = MemberRef {
                        address: module_ref.address.clone(),
                        module: module_ref.module.clone(),
                        member: member.name.clone(),
                    };
                    return self.resolve_member_call(&target);
                }

                let target = MemberRef {
                    address: function.module.address.clone(),
                    module: module.name.clone(),
                    member: member.name.clone(),
                };
                self.resolve_member_call(&target)
            }
            [address, module, member] => {
                let target = MemberRef {
                    address: Some(address.name.clone()),
                    module: module.name.clone(),
                    member: member.name.clone(),
                };
                self.resolve_member_call(&target)
            }
            _ => ResolvedCall::Unresolved("call target path shape is not a Move function path"),
        }
    }

    fn resolve_member_call(&self, member: &MemberRef) -> ResolvedCall {
        if let Some(target_id) =
            self.resolve_exact_function(member.address.as_deref(), &member.module, &member.member)
        {
            ResolvedCall::Local(target_id)
        } else {
            ResolvedCall::External(member.clone())
        }
    }

    fn resolve_type_apply(
        &mut self,
        context: &TypeContext,
        aliases: &AliasScope,
        name: &NameAccessChain,
        span: MoveSourceSpan,
        relationship: &str,
    ) -> Option<TypeUse> {
        let raw_type = name_access_to_string(name);
        let parts = name_access_parts(name);
        let type_ref = match parts.as_slice() {
            [single] if BUILTIN_TYPES.contains(&single.name.as_str()) => {
                return Some(TypeUse {
                    id: builtin_type_id(&single.name),
                });
            }
            [single] => {
                if let Some(type_parameter) = context.type_parameters.get(&single.name) {
                    return Some(TypeUse {
                        id: type_parameter.clone(),
                    });
                }

                if let Some(member) = aliases.member_aliases.get(&single.name) {
                    member.clone()
                } else if let Some(local_type) = self.resolve_exact_type(
                    context.module.address.as_deref(),
                    &context.module.module_name,
                    &single.name,
                ) {
                    return Some(TypeUse { id: local_type });
                } else {
                    self.record_unresolved_type(
                        context,
                        raw_type,
                        relationship,
                        span,
                        "single-name type did not match a builtin, type parameter, local type, or imported member",
                    );
                    return None;
                }
            }
            [module, member] => {
                if let Some(module_ref) = aliases.module_aliases.get(&module.name) {
                    MemberRef {
                        address: module_ref.address.clone(),
                        module: module_ref.module.clone(),
                        member: member.name.clone(),
                    }
                } else {
                    MemberRef {
                        address: context.module.address.clone(),
                        module: module.name.clone(),
                        member: member.name.clone(),
                    }
                }
            }
            [address, module, member] => MemberRef {
                address: Some(address.name.clone()),
                module: module.name.clone(),
                member: member.name.clone(),
            },
            _ => {
                self.record_unresolved_type(
                    context,
                    raw_type,
                    relationship,
                    span,
                    "type path shape is not supported by the graph resolver",
                );
                return None;
            }
        };

        if let Some(type_id) = self.resolve_exact_type(
            type_ref.address.as_deref(),
            &type_ref.module,
            &type_ref.member,
        ) {
            Some(TypeUse { id: type_id })
        } else {
            Some(TypeUse {
                id: self.ensure_external_type_node(
                    "datatype",
                    type_ref.address.as_deref(),
                    &type_ref.module,
                    &type_ref.member,
                    "source",
                ),
            })
        }
    }

    fn resolve_exact_function(
        &self,
        address: Option<&str>,
        module: &str,
        function: &str,
    ) -> Option<String> {
        let key = (
            address.map(str::to_string),
            module.to_string(),
            function.to_string(),
        );

        self.function_exact.get(&key).cloned().or_else(|| {
            address.is_none().then(|| {
                self.function_by_module_member
                    .get(&(module.to_string(), function.to_string()))
                    .and_then(|matches| {
                        if matches.len() == 1 {
                            matches.iter().next().cloned()
                        } else {
                            None
                        }
                    })
            })?
        })
    }

    fn resolve_exact_type(
        &self,
        address: Option<&str>,
        module: &str,
        name: &str,
    ) -> Option<String> {
        let key = (
            address.map(str::to_string),
            module.to_string(),
            name.to_string(),
        );

        self.type_exact.get(&key).cloned().or_else(|| {
            address.is_none().then(|| {
                self.type_by_module_member
                    .get(&(module.to_string(), name.to_string()))
                    .and_then(|matches| {
                        if matches.len() == 1 {
                            matches.iter().next().cloned()
                        } else {
                            None
                        }
                    })
            })?
        })
    }

    fn ensure_external_call_node(
        &mut self,
        target: &MemberRef,
        raw_target: &str,
        source: &str,
    ) -> String {
        let id = external_function_id(target.address.as_deref(), &target.module, &target.member);

        self.call_nodes
            .entry(id.clone())
            .or_insert_with(|| MoveCallGraphNode {
                id: id.clone(),
                package_name: None,
                package_path: None,
                address: target.address.clone(),
                module_name: target.module.clone(),
                function_name: target.member.clone(),
                qualified_name: raw_target.to_string(),
                file_path: None,
                visibility: "unknown".to_string(),
                is_entry: false,
                is_transaction_callable: false,
                attributes: Vec::new(),
                signature: None,
                span: None,
                is_external: true,
                source: source.to_string(),
            });

        id
    }

    fn ensure_unresolved_call_node(&mut self, raw_target: &str) -> String {
        let id = unresolved_call_id(raw_target);
        let function_name = raw_target
            .trim_start_matches('.')
            .rsplit("::")
            .next()
            .unwrap_or(raw_target)
            .to_string();
        let module_name = raw_target
            .trim_start_matches('.')
            .rsplit_once("::")
            .and_then(|(prefix, _)| prefix.rsplit("::").next())
            .unwrap_or("unresolved")
            .to_string();

        self.call_nodes
            .entry(id.clone())
            .or_insert_with(|| MoveCallGraphNode {
                id: id.clone(),
                package_name: None,
                package_path: None,
                address: None,
                module_name,
                function_name,
                qualified_name: raw_target.to_string(),
                file_path: None,
                visibility: "unknown".to_string(),
                is_entry: false,
                is_transaction_callable: false,
                attributes: Vec::new(),
                signature: None,
                span: None,
                is_external: false,
                source: "unresolved".to_string(),
            });

        id
    }

    fn ensure_external_type_node(
        &mut self,
        kind: &str,
        address: Option<&str>,
        module: &str,
        name: &str,
        source: &str,
    ) -> String {
        let id = external_type_id(kind, address, module, name);
        let canonical_address = canonical_address(&self.address_mapping, address);
        let abilities =
            well_known_external_type_abilities(address, canonical_address.as_deref(), module, name);

        self.type_nodes
            .entry(id.clone())
            .or_insert_with(|| MoveTypeGraphNode {
                id: id.clone(),
                kind: kind.to_string(),
                package_name: None,
                package_path: None,
                address: address.map(str::to_string),
                canonical_address,
                module_name: Some(module.to_string()),
                name: name.to_string(),
                qualified_name: qualified_member(address, module, name),
                file_path: None,
                abilities,
                type_parameters: well_known_type_parameters(module, name),
                attributes: Vec::new(),
                span: None,
                source: source.to_string(),
                is_external: true,
            });

        id
    }

    fn generic_argument_name_for(&self, owner_type_id: &str, index: usize) -> Option<String> {
        self.type_nodes
            .get(owner_type_id)
            .and_then(|node| node.type_parameters.get(index))
            .map(|parameter| parameter.name.clone())
            .or_else(|| {
                if owner_type_id == builtin_type_id("vector") && index == 0 {
                    Some("T".to_string())
                } else {
                    None
                }
            })
    }

    fn record_call_edge(&mut self, input: CallEdgeInput) {
        let key = CallEdgeKey {
            source: input.source.clone(),
            target: input.target.clone(),
            call_kind: input.call_kind.clone(),
            raw_target: input.raw_target.clone(),
            type_arguments: input.type_arguments.clone(),
        };
        let edge = self
            .call_edges
            .entry(key)
            .or_insert_with(|| MoveCallGraphEdge {
                source: input.source,
                target: input.target,
                call_kind: input.call_kind,
                confidence: input.confidence,
                call_count: 0,
                raw_target: input.raw_target,
                type_arguments: input.type_arguments,
                source_spans: Vec::new(),
                is_external: input.is_external,
                is_resolved: input.is_resolved,
            });

        edge.call_count += 1;
        edge.source_spans.push(input.span);
    }

    fn record_type_edge(&mut self, input: TypeEdgeInput) {
        let key = TypeEdgeKey {
            source: input.source.clone(),
            target: input.target.clone(),
            relationship: input.relationship.clone(),
            field_name: input.field_name.clone(),
            variant_name: input.variant_name.clone(),
            function_name: input.function_name.clone(),
            parameter_name: input.parameter_name.clone(),
            type_argument_index: input.type_argument_index,
            is_mutable: input.is_mutable,
            is_reference: input.is_reference,
            declaring_type_id: input.declaring_type_id.clone(),
            declaring_field_name: input.declaring_field_name.clone(),
            type_argument_name: input.type_argument_name.clone(),
        };
        let evidence = if input.evidence.is_empty() {
            default_type_edge_evidence(&input)
        } else {
            input.evidence
        };
        let edge = self
            .type_edges
            .entry(key)
            .or_insert_with(|| MoveTypeGraphEdge {
                source: input.source,
                target: input.target,
                relationship: input.relationship,
                field_name: input.field_name,
                variant_name: input.variant_name,
                function_name: input.function_name,
                parameter_name: input.parameter_name,
                type_argument_index: input.type_argument_index,
                is_mutable: input.is_mutable,
                is_reference: input.is_reference,
                type_expression: input.type_expression,
                declaring_type_id: input.declaring_type_id,
                declaring_field_name: input.declaring_field_name,
                type_argument_name: input.type_argument_name,
                source_spans: Vec::new(),
                confidence: input.confidence,
                evidence,
            });

        edge.source_spans.push(input.span);
    }

    fn record_unresolved_call(
        &mut self,
        function: &FunctionContext,
        raw_target: String,
        call_kind: &str,
        span: MoveSourceSpan,
        reason: &str,
    ) {
        let key = UnresolvedCallKey {
            source: function.function_id.clone(),
            raw_target: raw_target.clone(),
            call_kind: call_kind.to_string(),
            file_path: function.module.file_path.clone(),
            reason: reason.to_string(),
        };
        let unresolved = self
            .unresolved_calls
            .entry(key)
            .or_insert_with(|| MoveUnresolvedCall {
                source: function.function_id.clone(),
                raw_target,
                call_kind: call_kind.to_string(),
                file_path: function.module.file_path.clone(),
                spans: Vec::new(),
                reason: reason.to_string(),
            });

        unresolved.spans.push(span);
    }

    fn record_unresolved_type(
        &mut self,
        context: &TypeContext,
        raw_type: String,
        relationship: &str,
        span: MoveSourceSpan,
        reason: &str,
    ) {
        let key = UnresolvedTypeKey {
            source: context.owner_id.clone(),
            raw_type: raw_type.clone(),
            context: relationship.to_string(),
            file_path: context.module.file_path.clone(),
            reason: reason.to_string(),
        };
        let unresolved = self
            .unresolved_types
            .entry(key)
            .or_insert_with(|| MoveUnresolvedType {
                source: context.owner_id.clone(),
                raw_type,
                context: relationship.to_string(),
                file_path: context.module.file_path.clone(),
                spans: Vec::new(),
                reason: reason.to_string(),
            });

        unresolved.spans.push(span);
    }

}
