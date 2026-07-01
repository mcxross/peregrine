impl GraphBuilder {
    fn index_source_modules(&mut self, modules: &[SourceModule]) {
        for module in modules {
            let module_context = module_context(module);

            for member in &module.module.members {
                match member {
                    ModuleMember::Function(function) => {
                        self.index_function(&module_context, function);
                    }
                    ModuleMember::Struct(move_struct) => {
                        self.index_struct(&module_context, move_struct);
                    }
                    ModuleMember::Enum(move_enum) => {
                        self.index_enum(&module_context, move_enum);
                    }
                    _ => {}
                }
            }
        }

        self.index_builtin_types();
    }

    fn collect_source_relationships(&mut self, modules: &[SourceModule]) {
        for module in modules {
            let mut module_context = module_context(module);
            module_context.aliases = module_aliases(&module.module, &module_context.address);

            for member in &module.module.members {
                match member {
                    ModuleMember::Function(function) => {
                        self.collect_function(&module_context, function);
                    }
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

    fn index_function(&mut self, module: &ModuleContext, function: &Function) {
        let visibility = function_visibility_name(&function.visibility);
        let is_entry = function.entry.is_some();
        let id = function_id(
            Some(&module.package_path),
            module.address.as_deref(),
            &module.module_name,
            function.name.0.value.as_ref(),
        );
        let node = MoveCallGraphNode {
            id: id.clone(),
            package_name: Some(module.package_name.clone()),
            package_path: Some(module.package_path.clone()),
            address: module.address.clone(),
            module_name: module.module_name.clone(),
            function_name: function.name.0.value.to_string(),
            qualified_name: qualified_member(
                module.address.as_deref(),
                &module.module_name,
                function.name.0.value.as_ref(),
            ),
            file_path: Some(module.file_path.clone()),
            visibility: visibility.clone(),
            is_entry,
            is_transaction_callable: is_entry || visibility == "public",
            attributes: ast_attributes(&function.attributes, &module.source),
            signature: Some(ast_function_signature(function, &module.source)),
            span: Some(source_span(
                &module.source,
                &module.file_path,
                function.loc.start() as usize,
                function.loc.end() as usize,
            )),
            is_external: false,
            source: "source".to_string(),
        };

        self.function_exact.insert(
            (
                module.address.clone(),
                module.module_name.clone(),
                node.function_name.clone(),
            ),
            id.clone(),
        );
        self.function_by_module_member
            .entry((module.module_name.clone(), node.function_name.clone()))
            .or_default()
            .insert(id.clone());
        self.state_nodes
            .insert(id.clone(), state_node_from_call_node(&node));
        self.call_nodes.insert(id, node);
    }

    fn index_struct(&mut self, module: &ModuleContext, move_struct: &StructDefinition) {
        let name = move_struct.name.0.value.to_string();
        let id = type_id(
            "struct",
            module.address.as_deref(),
            &module.module_name,
            &name,
        );
        let node = MoveTypeGraphNode {
            id: id.clone(),
            kind: "struct".to_string(),
            package_name: Some(module.package_name.clone()),
            package_path: Some(module.package_path.clone()),
            address: module.address.clone(),
            canonical_address: canonical_address(&self.address_mapping, module.address.as_deref()),
            module_name: Some(module.module_name.clone()),
            name: name.clone(),
            qualified_name: qualified_member(module.address.as_deref(), &module.module_name, &name),
            file_path: Some(module.file_path.clone()),
            abilities: move_struct
                .abilities
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            type_parameters: struct_type_parameters(move_struct),
            attributes: ast_attributes(&move_struct.attributes, &module.source),
            span: Some(source_span(
                &module.source,
                &module.file_path,
                move_struct.loc.start() as usize,
                move_struct.loc.end() as usize,
            )),
            source: "source".to_string(),
            is_external: false,
        };

        self.type_exact.insert(
            (
                module.address.clone(),
                module.module_name.clone(),
                name.clone(),
            ),
            id.clone(),
        );
        self.type_by_module_member
            .entry((module.module_name.clone(), name))
            .or_default()
            .insert(id.clone());
        if is_state_type_node(&node) {
            self.state_nodes
                .insert(id.clone(), state_node_from_type_node(&node, "stateType"));
        }
        self.type_nodes.insert(id, node);
    }

    fn index_enum(&mut self, module: &ModuleContext, move_enum: &EnumDefinition) {
        let name = move_enum.name.0.value.to_string();
        let id = type_id(
            "enum",
            module.address.as_deref(),
            &module.module_name,
            &name,
        );
        let node = MoveTypeGraphNode {
            id: id.clone(),
            kind: "enum".to_string(),
            package_name: Some(module.package_name.clone()),
            package_path: Some(module.package_path.clone()),
            address: module.address.clone(),
            canonical_address: canonical_address(&self.address_mapping, module.address.as_deref()),
            module_name: Some(module.module_name.clone()),
            name: name.clone(),
            qualified_name: qualified_member(module.address.as_deref(), &module.module_name, &name),
            file_path: Some(module.file_path.clone()),
            abilities: move_enum
                .abilities
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            type_parameters: enum_type_parameters(move_enum),
            attributes: ast_attributes(&move_enum.attributes, &module.source),
            span: Some(source_span(
                &module.source,
                &module.file_path,
                move_enum.loc.start() as usize,
                move_enum.loc.end() as usize,
            )),
            source: "source".to_string(),
            is_external: false,
        };

        self.type_exact.insert(
            (
                module.address.clone(),
                module.module_name.clone(),
                name.clone(),
            ),
            id.clone(),
        );
        self.type_by_module_member
            .entry((module.module_name.clone(), name))
            .or_default()
            .insert(id.clone());
        if is_state_type_node(&node) {
            self.state_nodes
                .insert(id.clone(), state_node_from_type_node(&node, "stateType"));
        }
        self.type_nodes.insert(id, node);
    }

    fn index_builtin_types(&mut self) {
        for builtin in BUILTIN_TYPES {
            let id = builtin_type_id(builtin);

            self.type_nodes
                .entry(id.clone())
                .or_insert_with(|| MoveTypeGraphNode {
                    id,
                    kind: "builtin".to_string(),
                    package_name: None,
                    package_path: None,
                    address: None,
                    canonical_address: None,
                    module_name: None,
                    name: (*builtin).to_string(),
                    qualified_name: (*builtin).to_string(),
                    file_path: None,
                    abilities: builtin_abilities(builtin),
                    type_parameters: builtin_type_parameters(builtin),
                    attributes: builtin_attributes(builtin),
                    span: None,
                    source: "builtin".to_string(),
                    is_external: false,
                });
        }
    }

}
