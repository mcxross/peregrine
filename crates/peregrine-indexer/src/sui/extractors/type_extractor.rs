use serde_json::Value;

use crate::core::{logical_id, FieldInfo, SourceSpan, TypeDef, TypeKind};

pub fn type_def_from_shape(
    package_id: &str,
    module_id: &str,
    module_full_name: &str,
    name: &str,
    value: &Value,
    kind: TypeKind,
    source_span: SourceSpan,
) -> TypeDef {
    let full_name = format!("{module_full_name}::{name}");
    let type_id = logical_id("type", [package_id, &full_name]);
    let fields = fields_from_shape(package_id, module_id, &type_id, value, source_span.clone());
    TypeDef {
        id: type_id,
        package_id: package_id.to_string(),
        module_id: module_id.to_string(),
        name: name.to_string(),
        full_name,
        kind,
        abilities: string_array(value.get("abilities")),
        type_parameters: string_array(
            value
                .get("type_parameters")
                .or_else(|| value.get("type_params")),
        ),
        fields,
        docs: value
            .get("docs")
            .or_else(|| value.get("doc"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        attributes: string_array(value.get("attributes")),
        source_span,
    }
}

pub fn fields_from_shape(
    package_id: &str,
    module_id: &str,
    type_id: &str,
    value: &Value,
    source_span: SourceSpan,
) -> Vec<FieldInfo> {
    value
        .get("fields")
        .and_then(|fields| fields.get("fields").or(Some(fields)))
        .and_then(Value::as_object)
        .into_iter()
        .flat_map(|fields| fields.iter())
        .filter_map(|(name, field)| {
            let type_name = field
                .get("type_")
                .or_else(|| field.get("type"))
                .map(type_value_to_string)?;
            Some(FieldInfo {
                id: logical_id("field", [type_id, name]),
                package_id: package_id.to_string(),
                module_id: module_id.to_string(),
                type_id: type_id.to_string(),
                name: name.clone(),
                type_name,
                source_span: source_span.clone(),
            })
        })
        .collect()
}

pub fn type_value_to_string(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Object(map) => {
            if let Some(value) = map
                .get("type_")
                .or_else(|| map.get("type"))
                .or_else(|| map.get("Datatype"))
                .or_else(|| map.get("Struct"))
                .or_else(|| map.get("Reference"))
            {
                type_value_to_string(value)
            } else if let Some(value) = map.get("MutableReference") {
                format!("&mut {}", type_value_to_string(value))
            } else if let Some(value) = map.get("Vector") {
                format!("vector<{}>", type_value_to_string(value))
            } else if let Some(name) = map.get("name").and_then(Value::as_str) {
                name.to_string()
            } else {
                serde_json::to_string(value).unwrap_or_default()
            }
        }
        Value::Array(values) => values
            .iter()
            .map(type_value_to_string)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::Null => "unknown".to_string(),
    }
}

fn string_array(value: Option<&Value>) -> Vec<String> {
    value
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(type_value_to_string)
        .collect()
}
