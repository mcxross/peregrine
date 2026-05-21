fn split_top_level(source: &str, delimiter: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut angle_depth = 0_i32;
    let mut paren_depth = 0_i32;

    for (index, character) in source.char_indices() {
        match character {
            '<' => angle_depth += 1,
            '>' => angle_depth -= 1,
            '(' => paren_depth += 1,
            ')' => paren_depth -= 1,
            _ if character == delimiter && angle_depth == 0 && paren_depth == 0 => {
                parts.push(source[start..index].trim());
                start = index + character.len_utf8();
            }
            _ => {}
        }
    }

    parts.push(source[start..].trim());
    parts
}

fn source_contains_identifier(source: &str, identifier: &str) -> bool {
    source
        .split(|character: char| !is_identifier_character(character))
        .any(|token| token == identifier)
}

fn is_identifier_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn body_calls_function(
    body: &str,
    caller_module: &str,
    callee_module: &str,
    callee_name: &str,
) -> bool {
    let body = function_body_block(body);
    let qualified = format!("{callee_module}::{callee_name}");
    let qualified_call =
        body.contains(&format!("{qualified}(")) || body.contains(&format!("{qualified}<"));
    let same_module_call = caller_module == callee_module
        && (body.contains(&format!("{callee_name}(")) || body.contains(&format!("{callee_name}<")));

    qualified_call || same_module_call
}

fn is_delete_signal(lower_body: &str) -> bool {
    lower_body.contains(".delete(")
        || lower_body.contains("object::delete")
        || lower_body.contains("id.delete")
        || lower_body.contains("uid.delete")
}

fn capability_like_name(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    name.ends_with("cap")
        || name.ends_with("capability")
        || name.contains("_cap")
        || name.contains("admin")
        || name.contains("authority")
        || name.contains("owner")
        || name.contains("publisher")
        || name.contains("operator")
        || name.contains("guardian")
        || name.contains("treasury")
}

fn receipt_or_position_like(name: &str) -> bool {
    let name = name.to_ascii_lowercase();

    ["receipt", "position", "ticket", "order", "session"]
        .iter()
        .any(|term| name.contains(term))
}
