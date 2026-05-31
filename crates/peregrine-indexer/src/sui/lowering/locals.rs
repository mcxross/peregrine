use move_binary_format::file_format::{CompiledModule, SignatureToken};

use crate::core::{LocalInfo, PackageId, SourceSpan, logical_id};

pub fn lower_locals(
    module: &CompiledModule,
    package_id: &PackageId,
    function_id: &str,
    local_tokens: &[SignatureToken],
    source_span: SourceSpan,
) -> Vec<LocalInfo> {
    local_tokens
        .iter()
        .enumerate()
        .map(|(index, token)| LocalInfo {
            id: logical_id("local", [function_id, &index.to_string()]),
            package_id: package_id.clone(),
            function_id: function_id.to_string(),
            name: format!("local_{index}"),
            type_name: Some(signature_token_label(module, token)),
            index_in_function: Some(index),
            source_span: source_span.clone(),
        })
        .collect()
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
        other => format!("{other:?}"),
    }
}
