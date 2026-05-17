use serde_json::Value;

use crate::core::{logical_id, FunctionInfo, FunctionParameter, FunctionVisibility, SourceSpan};
use crate::sui::extractors::type_extractor::type_value_to_string;

pub fn function_from_shape(
    package_id: &str,
    module_id: &str,
    module_full_name: &str,
    name: &str,
    value: &Value,
    source_span: SourceSpan,
) -> FunctionInfo {
    let full_name = format!("{module_full_name}::{name}");
    let function_id = logical_id("function", [package_id, &full_name]);
    FunctionInfo {
        id: function_id.clone(),
        package_id: package_id.to_string(),
        module_id: module_id.to_string(),
        name: name.to_string(),
        full_name,
        visibility: visibility(value.get("visibility")),
        is_entry: value.get("entry").and_then(Value::as_bool).unwrap_or(false),
        is_native: value
            .get("is_native")
            .or_else(|| value.get("native"))
            .and_then(Value::as_bool)
            .unwrap_or(false),
        type_parameters: array_types(
            value
                .get("type_parameters")
                .or_else(|| value.get("type_params")),
        ),
        parameters: parameters(&function_id, value),
        returns: array_types(value.get("return_").or_else(|| value.get("returns"))),
        acquires: array_types(value.get("acquires")),
        docs: value
            .get("docs")
            .or_else(|| value.get("doc"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        attributes: array_types(value.get("attributes")),
        source_span,
    }
}

pub fn visibility(value: Option<&Value>) -> FunctionVisibility {
    match value {
        Some(Value::String(value)) if value.contains("Friend") => FunctionVisibility::PublicFriend,
        Some(Value::String(value)) if value.contains("Package") => {
            FunctionVisibility::PublicPackage
        }
        Some(Value::String(value)) if value.contains("Public") => FunctionVisibility::Public,
        Some(Value::String(value)) if value.contains("Native") => FunctionVisibility::Native,
        Some(Value::Object(value)) if value.contains_key("Public") => FunctionVisibility::Public,
        Some(Value::Object(value)) if value.contains_key("Friend") => {
            FunctionVisibility::PublicFriend
        }
        Some(Value::Object(value)) if value.contains_key("Package") => {
            FunctionVisibility::PublicPackage
        }
        Some(Value::Object(value)) if value.contains_key("Native") => FunctionVisibility::Native,
        _ => FunctionVisibility::Private,
    }
}

fn parameters(function_id: &str, value: &Value) -> Vec<FunctionParameter> {
    value
        .get("parameters")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, parameter)| FunctionParameter {
            id: logical_id("parameter", [function_id, &index.to_string()]),
            name: parameter
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned),
            type_name: parameter
                .get("type_")
                .or_else(|| parameter.get("type"))
                .map(type_value_to_string)
                .unwrap_or_else(|| type_value_to_string(parameter)),
            index,
        })
        .collect()
}

fn array_types(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(type_value_to_string)
        .collect()
}
