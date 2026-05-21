fn function_parameters(signature: &str) -> Option<&str> {
    let start = signature.find('(')?;
    let mut depth = 0_i32;

    for (offset, character) in signature[start..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;

                if depth == 0 {
                    return Some(&signature[start + 1..start + offset]);
                }
            }
            _ => {}
        }
    }

    None
}

fn function_returns_type(signature: &str, type_name: &str) -> bool {
    if type_name.is_empty() {
        return false;
    }

    let Some(close_parameters) = signature.rfind(')') else {
        return false;
    };

    let after_parameters = signature[close_parameters + 1..].trim_start();
    let Some(return_type) = after_parameters.strip_prefix(':') else {
        return false;
    };

    type_reference_matches(return_type, type_name)
}

fn type_reference_matches(source: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    source
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '&' | ',' | ':' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ';' | '='
                )
        })
        .any(|token| token == short_name || token == type_name)
}

fn body_constructs_type(body: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    body.contains(&format!("{short_name} {{"))
        || body.contains(&format!("{short_name}<"))
        || body.contains(&format!("{type_name} {{"))
        || body.contains(&format!("{type_name}<"))
}

