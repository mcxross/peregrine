use crate::core::{
    is_neutral_tag, stable_id, FunctionInfo, FunctionVisibility, SemanticTag, SourceSpan, TypeDef,
};

pub fn function_tags(function: &FunctionInfo) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if function.is_entry && function.visibility == FunctionVisibility::Public {
        tags.push("public_entry_detected");
    }
    if function.visibility == FunctionVisibility::Public {
        tags.push("public_function_detected");
    }
    if function.visibility == FunctionVisibility::PublicFriend {
        tags.push("friend_function_detected");
    }
    if function.is_native {
        tags.push("native_function_detected");
    }
    if function.name == "init" {
        tags.push("init_function_detected");
    }
    if function
        .parameters
        .iter()
        .any(|parameter| parameter.type_name.contains("TxContext"))
    {
        tags.push("tx_context_parameter_detected");
    }
    if function
        .parameters
        .iter()
        .any(|parameter| parameter.type_name.contains("&mut"))
    {
        tags.push("mutable_reference_parameter_detected");
    }
    if !function.type_parameters.is_empty() {
        tags.push("generic_function_detected");
    }
    tags
}

pub fn type_tags(type_def: &TypeDef) -> Vec<&'static str> {
    let mut tags = Vec::new();
    if type_def.abilities.iter().any(|ability| ability == "key") {
        tags.push("ability_key_detected");
    }
    if type_def.abilities.iter().any(|ability| ability == "store") {
        tags.push("store_type_detected");
    }
    if type_def
        .fields
        .iter()
        .any(|field| field.name == "id" && field.type_name.contains("UID"))
    {
        tags.push("uid_field_detected");
        tags.push("key_object_type_detected");
    }
    let lower = type_def.name.to_ascii_lowercase();
    if lower.contains("cap") {
        tags.push("capability_named_type_detected");
    }
    if lower.contains("witness") {
        tags.push("witness_named_type_detected");
    }
    tags
}

pub fn operation_call_tag(target: &str) -> &'static str {
    let lower = target.to_ascii_lowercase();
    if lower.contains("::transfer::") {
        "transfer_api_call_detected"
    } else if lower.contains("::coin::") {
        "coin_api_call_detected"
    } else if lower.contains("::balance::") {
        "balance_api_call_detected"
    } else if lower.contains("::dynamic_field::") {
        "dynamic_field_api_call_detected"
    } else if lower.contains("::table::") {
        "table_api_call_detected"
    } else if lower.contains("::clock::") {
        "clock_api_call_detected"
    } else if lower.contains("::tx_context::") {
        "tx_context_api_call_detected"
    } else {
        "package_external_call_detected"
    }
}

pub fn build_tag(
    package_id: &str,
    target_id: &str,
    tag: &str,
    source_span: SourceSpan,
    metadata_json: Option<serde_json::Value>,
) -> Option<SemanticTag> {
    is_neutral_tag(tag).then(|| SemanticTag {
        id: stable_id("tag", [package_id, target_id, tag]),
        package_id: package_id.to_string(),
        target_id: target_id.to_string(),
        tag: tag.to_string(),
        source_span,
        metadata_json,
    })
}
