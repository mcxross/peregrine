fn module_context(module: &SourceModule) -> ModuleContext {
    ModuleContext {
        package_name: module.package_name.clone(),
        package_path: module.package_path.clone(),
        address: module.address.clone(),
        module_name: module.name.clone(),
        file_path: module.file_path.clone(),
        source: module.source.clone(),
        aliases: AliasScope::default(),
    }
}

fn module_aliases(module: &ModuleDefinition, current_address: &Option<String>) -> AliasScope {
    let mut aliases = AliasScope::default();

    for member in &module.members {
        if let ModuleMember::Use(use_decl) = member {
            apply_use_decl(
                &mut aliases,
                use_decl,
                current_address,
                &module.name.0.value.to_string(),
            );
        }
    }

    aliases
}

fn apply_use_decl(
    aliases: &mut AliasScope,
    use_decl: &UseDecl,
    current_address: &Option<String>,
    current_module: &str,
) {
    match &use_decl.use_ {
        Use::ModuleUse(module, module_use) => {
            apply_module_use(
                aliases,
                ModuleRef {
                    address: Some(leading_name_access_to_string(&module.value.address)),
                    module: module.value.module.0.value.to_string(),
                },
                module_use,
            );
        }
        Use::NestedModuleUses(address, modules) => {
            let address = leading_name_access_to_string(address);

            for (module, module_use) in modules {
                apply_module_use(
                    aliases,
                    ModuleRef {
                        address: Some(address.clone()),
                        module: module.0.value.to_string(),
                    },
                    module_use,
                );
            }
        }
        Use::Fun {
            function, method, ..
        } => {
            if let Some(target) =
                member_ref_from_name_access(function, current_address, current_module)
            {
                aliases
                    .method_aliases
                    .insert(method.value.to_string(), target);
            }
        }
        Use::Partial { .. } => {}
    }
}

fn apply_module_use(aliases: &mut AliasScope, module_ref: ModuleRef, module_use: &ModuleUse) {
    match module_use {
        ModuleUse::Module(alias) => {
            let alias = alias
                .map(|name| name.0.value.to_string())
                .unwrap_or_else(|| module_ref.module.clone());

            aliases.module_aliases.insert(alias, module_ref);
        }
        ModuleUse::Members(members) => {
            for (member, alias) in members {
                let alias = alias
                    .map(|name| name.value.to_string())
                    .unwrap_or_else(|| member.value.to_string());

                aliases.member_aliases.insert(
                    alias,
                    MemberRef {
                        address: module_ref.address.clone(),
                        module: module_ref.module.clone(),
                        member: member.value.to_string(),
                    },
                );
            }
        }
        ModuleUse::Partial { .. } => {}
    }
}

fn member_ref_from_name_access(
    name: &NameAccessChain,
    current_address: &Option<String>,
    current_module: &str,
) -> Option<MemberRef> {
    let parts = name_access_parts(name);

    match parts.as_slice() {
        [single] => Some(MemberRef {
            address: current_address.clone(),
            module: current_module.to_string(),
            member: single.name.clone(),
        }),
        [module, member] => Some(MemberRef {
            address: current_address.clone(),
            module: module.name.clone(),
            member: member.name.clone(),
        }),
        [address, module, member] => Some(MemberRef {
            address: Some(address.name.clone()),
            module: module.name.clone(),
            member: member.name.clone(),
        }),
        _ => None,
    }
}

fn name_access_parts(name: &NameAccessChain) -> Vec<NamePart> {
    match &name.value {
        NameAccessChain_::Single(entry) => vec![NamePart {
            name: entry.name.value.to_string(),
            is_macro: entry.is_macro.is_some(),
        }],
        NameAccessChain_::Path(path) => {
            let mut parts = vec![NamePart {
                name: leading_name_access_to_string(&path.root.name),
                is_macro: path.root.is_macro.is_some(),
            }];

            parts.extend(path.entries.iter().map(|entry| NamePart {
                name: entry.name.value.to_string(),
                is_macro: entry.is_macro.is_some(),
            }));
            parts
        }
    }
}

fn name_access_to_string(name: &NameAccessChain) -> String {
    name_access_parts(name)
        .into_iter()
        .map(|part| part.name)
        .collect::<Vec<_>>()
        .join("::")
}

fn name_access_local_name(name: &NameAccessChain) -> Option<String> {
    let parts = name_access_parts(name);

    match parts.as_slice() {
        [single] => Some(single.name.clone()),
        _ => None,
    }
}

fn name_access_type_arguments<'a>(name: &'a NameAccessChain, source: &str) -> Vec<String> {
    name_access_types(name)
        .into_iter()
        .map(|type_| ast_type_source(type_, source))
        .collect()
}

fn name_access_types(name: &NameAccessChain) -> Vec<&Type> {
    match &name.value {
        NameAccessChain_::Single(entry) => entry
            .tyargs
            .as_ref()
            .map(|types| types.value.iter().collect())
            .unwrap_or_default(),
        NameAccessChain_::Path(path) => {
            let mut types = path
                .root
                .tyargs
                .as_ref()
                .map(|types| types.value.iter().collect::<Vec<_>>())
                .unwrap_or_default();

            for entry in &path.entries {
                if let Some(entry_types) = &entry.tyargs {
                    types.extend(entry_types.value.iter());
                }
            }

            types
        }
    }
}

fn leading_name_access_to_string(access: &LeadingNameAccess) -> String {
    match &access.value {
        LeadingNameAccess_::AnonymousAddress(address) => format!("{address}"),
        LeadingNameAccess_::GlobalAddress(name) | LeadingNameAccess_::Name(name) => {
            name.value.to_string()
        }
    }
}

fn function_visibility_name(visibility: &Visibility) -> String {
    match visibility {
        Visibility::Public(_) => "public",
        Visibility::Friend(_) => "public(friend)",
        Visibility::Package(_) => "public(package)",
        Visibility::Internal => "private",
    }
    .to_string()
}

fn state_access_kind_for_parameter_type(type_: &Type) -> &'static str {
    match &type_.value {
        Type_::Ref(true, _) => "borrowMut",
        Type_::Ref(false, _) => "borrowImm",
        Type_::Multiple(types) => types
            .iter()
            .find_map(|type_| match state_access_kind_for_parameter_type(type_) {
                "move" => None,
                access_kind => Some(access_kind),
            })
            .unwrap_or("move"),
        _ => "move",
    }
}

fn is_state_type_node(node: &MoveTypeGraphNode) -> bool {
    !node.is_external
        && matches!(node.kind.as_str(), "struct" | "enum")
        && node
            .abilities
            .iter()
            .any(|ability| matches!(ability.as_str(), "key" | "store"))
}

fn state_node_from_call_node(node: &MoveCallGraphNode) -> MoveStateAccessGraphNode {
    MoveStateAccessGraphNode {
        id: node.id.clone(),
        kind: "function".to_string(),
        package_name: node.package_name.clone(),
        package_path: node.package_path.clone(),
        address: node.address.clone(),
        module_name: Some(node.module_name.clone()),
        name: node.function_name.clone(),
        qualified_name: node.qualified_name.clone(),
        file_path: node.file_path.clone(),
        abilities: Vec::new(),
        span: node.span.clone(),
        is_external: node.is_external,
        source: node.source.clone(),
    }
}

fn state_node_from_type_node(node: &MoveTypeGraphNode, kind: &str) -> MoveStateAccessGraphNode {
    MoveStateAccessGraphNode {
        id: node.id.clone(),
        kind: kind.to_string(),
        package_name: node.package_name.clone(),
        package_path: node.package_path.clone(),
        address: node.address.clone(),
        module_name: node.module_name.clone(),
        name: node.name.clone(),
        qualified_name: node.qualified_name.clone(),
        file_path: node.file_path.clone(),
        abilities: node.abilities.clone(),
        span: node.span.clone(),
        is_external: node.is_external,
        source: node.source.clone(),
    }
}

fn ability_name(ability: &Ability_) -> &'static str {
    match ability {
        Ability_::Copy => "copy",
        Ability_::Drop => "drop",
        Ability_::Store => "store",
        Ability_::Key => "key",
    }
}

fn builtin_abilities(name: &str) -> Vec<String> {
    match name {
        "address" | "bool" | "u8" | "u16" | "u32" | "u64" | "u128" | "u256" | "vector" => {
            copy_drop_store_abilities()
        }
        "signer" => vec!["drop".to_string()],
        _ => Vec::new(),
    }
}

fn struct_type_parameters(move_struct: &StructDefinition) -> Vec<MoveTypeParameter> {
    move_struct
        .type_parameters
        .iter()
        .map(|parameter| MoveTypeParameter {
            name: parameter.name.value.to_string(),
            abilities: parameter
                .constraints
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            is_phantom: parameter.is_phantom,
        })
        .collect()
}

fn enum_type_parameters(move_enum: &EnumDefinition) -> Vec<MoveTypeParameter> {
    move_enum
        .type_parameters
        .iter()
        .map(|parameter| MoveTypeParameter {
            name: parameter.name.value.to_string(),
            abilities: parameter
                .constraints
                .iter()
                .map(|ability| ability_name(&ability.value).to_string())
                .collect(),
            is_phantom: parameter.is_phantom,
        })
        .collect()
}

fn builtin_type_parameters(name: &str) -> Vec<MoveTypeParameter> {
    if name == "vector" {
        vec![MoveTypeParameter {
            name: "T".to_string(),
            abilities: Vec::new(),
            is_phantom: false,
        }]
    } else {
        Vec::new()
    }
}

fn builtin_attributes(name: &str) -> Vec<String> {
    if name == "vector" {
        vec!["conditional abilities".to_string()]
    } else {
        Vec::new()
    }
}

fn well_known_external_type_abilities(
    address: Option<&str>,
    canonical_address: Option<&str>,
    module: &str,
    name: &str,
) -> Vec<String> {
    if !is_framework_address(address) && !is_framework_address(canonical_address) {
        return Vec::new();
    }

    match (module, name) {
        ("object", "ID") => copy_drop_store_abilities(),
        ("object", "UID") => vec!["store".to_string()],
        ("table", "Table") => vec!["key".to_string(), "store".to_string()],
        _ => Vec::new(),
    }
}

fn well_known_type_parameters(module: &str, name: &str) -> Vec<MoveTypeParameter> {
    let parameter_names: &[&str] = match (module, name) {
        ("table", "Table") => &["K", "V"],
        ("coin", "Coin")
        | ("balance", "Balance")
        | ("bag", "Bag")
        | ("object_bag", "ObjectBag") => &["T"],
        ("vec_map", "VecMap") => &["K", "V"],
        ("vec_set", "VecSet") => &["K"],
        ("dynamic_field", "Field") | ("dynamic_object_field", "Wrapper") => &["Name", "Value"],
        _ => &[],
    };

    parameter_names
        .iter()
        .map(|name| MoveTypeParameter {
            name: (*name).to_string(),
            abilities: Vec::new(),
            is_phantom: false,
        })
        .collect()
}

fn copy_drop_store_abilities() -> Vec<String> {
    COPY_DROP_STORE_ABILITIES
        .iter()
        .map(|ability| (*ability).to_string())
        .collect()
}

fn default_type_edge_evidence(input: &TypeEdgeInput) -> Vec<String> {
    let mut evidence = Vec::new();

    if let Some(field_name) = &input.field_name {
        evidence.push(format!(
            "Field `{field_name}` declares this type relationship."
        ));
    } else if let Some(parameter_name) = &input.parameter_name {
        evidence.push(format!(
            "Function parameter `{parameter_name}` declares this type relationship."
        ));
    } else if let Some(function_name) = &input.function_name {
        evidence.push(format!(
            "Function `{function_name}` declares this type relationship."
        ));
    }

    if let Some(type_expression) = &input.type_expression {
        evidence.push(format!("Type expression: `{type_expression}`."));
    }

    if evidence.is_empty() {
        evidence.push(format!(
            "Syntactic `{}` relationship recorded by the Move parser.",
            input.relationship
        ));
    }

    evidence
}

fn type_usage_evidence(
    context: &TypeContext,
    target_id: &str,
    type_expression: &str,
    input: &TypeUsageInput,
) -> Vec<String> {
    let mut evidence = Vec::new();

    if let Some(field_name) = &input.field_name {
        evidence.push(format!(
            "Field `{field_name}` in `{}` is declared as `{type_expression}`.",
            context.owner_name.as_deref().unwrap_or(&context.owner_id)
        ));
    } else if let Some(parameter_name) = &input.parameter_name {
        evidence.push(format!(
            "Parameter `{parameter_name}` in `{}` is declared as `{type_expression}`.",
            context.owner_name.as_deref().unwrap_or(&context.owner_id)
        ));
    } else {
        evidence.push(format!(
            "`{}` references `{type_expression}`.",
            context.owner_name.as_deref().unwrap_or(&context.owner_id)
        ));
    }

    evidence.push(format!("Resolved target type id: `{target_id}`."));
    evidence
}

fn generic_argument_evidence(
    owner_type_id: &str,
    argument_name: Option<&str>,
    index: usize,
    argument_expression: &str,
    input: &TypeUsageInput,
) -> Vec<String> {
    let name = argument_name
        .map(|name| format!("`{name}`"))
        .unwrap_or_else(|| format!("index `{index}`"));
    let mut evidence = vec![format!(
        "Generic argument {name} of `{owner_type_id}` resolves to `{argument_expression}`."
    )];

    if let Some(field_name) = &input.field_name {
        evidence.push(format!("Argument appears inside field `{field_name}`."));
    }

    evidence
}

fn ast_attributes(attributes: &[Attributes], source: &str) -> Vec<String> {
    let mut names = Vec::new();

    for attribute_group in attributes {
        let snippet = source_for_range(
            source,
            attribute_group.loc.start() as usize,
            attribute_group.loc.end() as usize,
        )
        .unwrap_or_default();

        for attribute in &attribute_group.value.0 {
            let name = match &attribute.value {
                Attribute_::Mode { .. } => {
                    if snippet.contains("test_only") {
                        "test_only"
                    } else if snippet.contains("mode(test)") || snippet.contains("mode = test") {
                        "mode(test)"
                    } else {
                        attribute.value.attribute_name()
                    }
                }
                _ => attribute.value.attribute_name(),
            };

            names.push(name.to_string());
        }

        if snippet.contains("test_only") && !names.iter().any(|name| name == "test_only") {
            names.push("test_only".to_string());
        }
    }

    names.sort();
    names.dedup();
    names
}

fn ast_function_signature(function: &Function, source: &str) -> String {
    let start = function.loc.start() as usize;
    let end = match &function.body.value {
        FunctionBody_::Defined(_) => function.body.loc.start() as usize,
        FunctionBody_::Native => function.loc.end() as usize,
    };

    source_for_range(source, start, end)
        .unwrap_or_default()
        .trim()
        .trim_end_matches('{')
        .trim_end_matches(';')
        .trim()
        .to_string()
}

fn ast_type_source(type_: &Type, source: &str) -> String {
    source_for_range(source, type_.loc.start() as usize, type_.loc.end() as usize)
        .unwrap_or_default()
        .trim()
        .to_string()
}

fn qualified_member(address: Option<&str>, module: &str, member: &str) -> String {
    if let Some(address) = address {
        format!("{address}::{module}::{member}")
    } else {
        format!("{module}::{member}")
    }
}

fn canonical_address(
    address_mapping: &BTreeMap<String, String>,
    address: Option<&str>,
) -> Option<String> {
    address.and_then(|address| {
        address_mapping
            .get(address)
            .cloned()
            .or_else(|| address.starts_with("0x").then(|| address.to_string()))
    })
}

fn is_framework_address(address: Option<&str>) -> bool {
    let Some(address) = address.map(str::to_lowercase) else {
        return false;
    };

    matches!(address.as_str(), "std" | "sui" | "0x1" | "0x2")
        || address.ends_with("0000000000000000000000000000000000000000000000000000000000000001")
        || address.ends_with("0000000000000000000000000000000000000000000000000000000000000002")
}
