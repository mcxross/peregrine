use serde_json::Value;

use crate::core::{FieldInfo, SourceSpan, TypeDef, TypeKind, logical_id};

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
        abilities: string_array(value.get("abilities"))
            .into_iter()
            .map(|ability| ability.to_ascii_lowercase())
            .collect(),
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
            if let Some(value) = map.get("type_").or_else(|| map.get("type")) {
                type_value_to_string(value)
            } else if let Some(value) = map.get("Reference") {
                reference_type_to_string(value)
            } else if let Some(value) = map.get("NamedTypeParameter").and_then(Value::as_str) {
                value.to_string()
            } else if let Some(value) = map.get("argument") {
                type_value_to_string(value)
            } else if let Some(value) = map.get("Datatype").or_else(|| map.get("Struct")) {
                type_value_to_string(value)
            } else if let Some(value) = map.get("MutableReference") {
                format!("&mut {}", type_value_to_string(value))
            } else if let Some(value) = map.get("Vector").or_else(|| map.get("vector")) {
                format!("vector<{}>", type_value_to_string(value))
            } else if let Some(name) = map.get("name").and_then(Value::as_str) {
                if let Some(constraints) = constraints_to_string(map.get("constraints")) {
                    let name = if map.get("phantom").and_then(Value::as_bool).unwrap_or(false) {
                        format!("phantom {name}")
                    } else {
                        name.to_string()
                    };
                    return format!("{name}: {constraints}");
                }
                if let Some(module) = datatype_module_to_string(map.get("module")) {
                    let mut rendered = format!("{module}::{name}");
                    if let Some(arguments) = type_arguments_to_string(map.get("type_arguments")) {
                        rendered.push('<');
                        rendered.push_str(&arguments);
                        rendered.push('>');
                    }
                    return rendered;
                }
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

fn constraints_to_string(value: Option<&Value>) -> Option<String> {
    let constraints = value?
        .as_array()?
        .iter()
        .map(type_value_to_string)
        .map(|constraint| constraint.to_ascii_lowercase())
        .collect::<Vec<_>>();
    (!constraints.is_empty()).then(|| constraints.join(" + "))
}

fn type_arguments_to_string(value: Option<&Value>) -> Option<String> {
    let arguments = value?
        .as_array()?
        .iter()
        .map(type_value_to_string)
        .collect::<Vec<_>>();
    (!arguments.is_empty()).then(|| arguments.join(", "))
}

fn reference_type_to_string(value: &Value) -> String {
    match value {
        Value::Array(items) if items.len() == 2 => {
            let mutable = items.first().and_then(Value::as_bool).unwrap_or(false);
            let inner = items
                .get(1)
                .map(type_value_to_string)
                .unwrap_or_else(|| "unknown".to_string());
            if mutable {
                format!("&mut {inner}")
            } else {
                format!("&{inner}")
            }
        }
        _ => format!("&{}", type_value_to_string(value)),
    }
}

fn datatype_module_to_string(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::String(module)) => Some(module.clone()),
        Some(Value::Object(module)) => {
            let address = module.get("address").and_then(Value::as_str)?;
            let name = module.get("name").and_then(Value::as_str)?;
            Some(format!("{address}::{name}"))
        }
        _ => None,
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
