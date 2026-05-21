fn module_interface_source(module: &CompiledModule) -> String {
    let mut source = String::new();
    let module_id = module.self_id();
    let module_address = module_id.address().to_hex_literal();
    let module_name = module_id.name();

    source.push_str("// Decompiled interface generated from on-chain bytecode.\n");
    source.push_str("// Instruction-level disassembly is available in decompiled/*.moveasm.\n\n");
    source.push_str(&format!("module {module_address}::{module_name} {{\n"));

    for struct_def in module.struct_defs() {
        source.push('\n');
        source.push_str(&struct_interface_source(module, struct_def));
    }

    for function_def in module.function_defs() {
        source.push('\n');
        source.push_str(&function_interface_source(module, function_def));
    }

    source.push_str("}\n");
    source
}

fn struct_interface_source(
    module: &CompiledModule,
    struct_def: &move_binary_format::file_format::StructDefinition,
) -> String {
    let handle = module.datatype_handle_at(struct_def.struct_handle);
    let name = module.identifier_at(handle.name);
    let type_parameters = datatype_type_parameters_source(&handle.type_parameters);
    let abilities = ability_list_source(handle.abilities);
    let ability_clause = if abilities.is_empty() {
        String::new()
    } else {
        format!(" has {}", abilities.join(", "))
    };

    match &struct_def.field_information {
        StructFieldInformation::Native => {
            format!("    public struct {name}{type_parameters}{ability_clause} {{}}\n")
        }
        StructFieldInformation::Declared(fields) if fields.is_empty() => {
            format!("    public struct {name}{type_parameters}{ability_clause} {{}}\n")
        }
        StructFieldInformation::Declared(fields) => {
            let mut source =
                format!("    public struct {name}{type_parameters}{ability_clause} {{\n");

            for field in fields {
                let field_name = module.identifier_at(field.name);
                let field_type = source_signature_token_label(module, &field.signature.0);
                source.push_str(&format!("        {field_name}: {field_type},\n"));
            }

            source.push_str("    }\n");
            source
        }
    }
}

fn function_interface_source(
    module: &CompiledModule,
    function_def: &move_binary_format::file_format::FunctionDefinition,
) -> String {
    let handle = module.function_handle_at(function_def.function);
    let name = module.identifier_at(handle.name);
    let visibility = function_visibility_source(function_def.visibility);
    let entry = if function_def.is_entry { "entry " } else { "" };
    let type_parameters = function_type_parameters_source(&handle.type_parameters);
    let parameters = module
        .signature_at(handle.parameters)
        .0
        .iter()
        .enumerate()
        .map(|(index, token)| {
            format!(
                "arg{index}: {}",
                source_signature_token_label(module, token)
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    let returns = module
        .signature_at(handle.return_)
        .0
        .iter()
        .map(|token| source_signature_token_label(module, token))
        .collect::<Vec<_>>();
    let return_clause = match returns.as_slice() {
        [] => String::new(),
        [single] => format!(": {single}"),
        _ => format!(": ({})", returns.join(", ")),
    };
    let acquires = function_def
        .acquires_global_resources
        .iter()
        .map(|index| {
            let struct_def = module.struct_def_at(*index);
            let handle = module.datatype_handle_at(struct_def.struct_handle);
            module.identifier_at(handle.name).to_string()
        })
        .collect::<Vec<_>>();
    let acquires_clause = if acquires.is_empty() {
        String::new()
    } else {
        format!(" acquires {}", acquires.join(", "))
    };

    format!(
        "    {visibility}{entry}fun {name}{type_parameters}({parameters}){return_clause}{acquires_clause} {{\n        abort 0\n    }}\n"
    )
}

fn source_signature_token_label(module: &CompiledModule, token: &SignatureToken) -> String {
    match token {
        SignatureToken::Bool => "bool".to_string(),
        SignatureToken::U8 => "u8".to_string(),
        SignatureToken::U16 => "u16".to_string(),
        SignatureToken::U32 => "u32".to_string(),
        SignatureToken::U64 => "u64".to_string(),
        SignatureToken::U128 => "u128".to_string(),
        SignatureToken::U256 => "u256".to_string(),
        SignatureToken::Address => "address".to_string(),
        SignatureToken::Signer => "signer".to_string(),
        SignatureToken::Vector(inner) => {
            format!("vector<{}>", source_signature_token_label(module, inner))
        }
        SignatureToken::Reference(inner) => {
            format!("&{}", source_signature_token_label(module, inner))
        }
        SignatureToken::MutableReference(inner) => {
            format!("&mut {}", source_signature_token_label(module, inner))
        }
        SignatureToken::TypeParameter(index) => format!("T{index}"),
        SignatureToken::Datatype(index) => source_datatype_label(module, *index),
        SignatureToken::DatatypeInstantiation(instantiation) => {
            let (index, type_arguments) = &**instantiation;
            let arguments = type_arguments
                .iter()
                .map(|argument| source_signature_token_label(module, argument))
                .collect::<Vec<_>>()
                .join(", ");

            format!("{}<{arguments}>", source_datatype_label(module, *index))
        }
    }
}

fn source_datatype_label(
    module: &CompiledModule,
    index: move_binary_format::file_format::DatatypeHandleIndex,
) -> String {
    let handle = module.datatype_handle_at(index);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));

    format!(
        "{}::{}::{}",
        module_id.address().to_hex_literal(),
        module_id.name(),
        module.identifier_at(handle.name),
    )
}

fn function_visibility_source(visibility: Visibility) -> &'static str {
    match visibility {
        Visibility::Private => "",
        Visibility::Public => "public ",
        Visibility::Friend => "public(friend) ",
    }
}

fn datatype_type_parameters_source(
    parameters: &[move_binary_format::file_format::DatatypeTyParameter],
) -> String {
    let parameters = parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| {
            let mut source = if parameter.is_phantom {
                format!("phantom T{index}")
            } else {
                format!("T{index}")
            };
            let constraints = ability_constraints_source(parameter.constraints);

            if !constraints.is_empty() {
                source.push_str(": ");
                source.push_str(&constraints);
            }

            source
        })
        .collect::<Vec<_>>();

    type_parameter_list_source(parameters)
}

fn function_type_parameters_source(parameters: &[AbilitySet]) -> String {
    let parameters = parameters
        .iter()
        .enumerate()
        .map(|(index, constraints)| {
            let constraints = ability_constraints_source(*constraints);

            if constraints.is_empty() {
                format!("T{index}")
            } else {
                format!("T{index}: {constraints}")
            }
        })
        .collect::<Vec<_>>();

    type_parameter_list_source(parameters)
}

fn type_parameter_list_source(parameters: Vec<String>) -> String {
    if parameters.is_empty() {
        String::new()
    } else {
        format!("<{}>", parameters.join(", "))
    }
}

fn ability_constraints_source(abilities: AbilitySet) -> String {
    ability_list_source(abilities).join(" + ")
}

fn ability_list_source(abilities: AbilitySet) -> Vec<String> {
    abilities
        .into_iter()
        .map(|ability| ability.to_string())
        .collect()
}
