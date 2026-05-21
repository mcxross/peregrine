fn opcode_name(instruction: &Bytecode) -> String {
    let detail = format!("{instruction:?}");
    let end = detail.find(['(', ' ']).unwrap_or(detail.len());
    detail[..end].to_string()
}

fn signature_token_label(module: &CompiledModule, token: &SignatureToken) -> String {
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
            format!("vector<{}>", signature_token_label(module, inner))
        }
        SignatureToken::Reference(inner) => format!("&{}", signature_token_label(module, inner)),
        SignatureToken::MutableReference(inner) => {
            format!("&mut {}", signature_token_label(module, inner))
        }
        SignatureToken::TypeParameter(index) => format!("T{index}"),
        SignatureToken::Datatype(index) => datatype_label(module, *index),
        SignatureToken::DatatypeInstantiation(instantiation) => {
            let (index, type_arguments) = &**instantiation;
            let arguments = type_arguments
                .iter()
                .map(|argument| signature_token_label(module, argument))
                .collect::<Vec<_>>()
                .join(", ");

            format!("{}<{arguments}>", datatype_label(module, *index))
        }
    }
}

fn datatype_label(
    module: &CompiledModule,
    index: move_binary_format::file_format::DatatypeHandleIndex,
) -> String {
    let handle = module.datatype_handle_at(index);
    let module_id = module.module_id_for_handle(module.module_handle_at(handle.module));

    format!(
        "{}::{}::{}",
        module_id.address().short_str_lossless(),
        module_id.name(),
        module.identifier_at(handle.name),
    )
}
