fn function_parameters_contain_type(signature: &str, type_name: &str) -> bool {
    let Some(parameters) = function_parameters(signature) else {
        return false;
    };

    type_reference_matches(parameters, type_name)
}

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

fn type_reference_matches(source: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    source
        .split(|character: char| {
            character.is_whitespace()
                || matches!(
                    character,
                    '&' | ',' | ':' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ';'
                )
        })
        .any(|token| token == short_name || token == type_name)
}

fn privileged_function(function: &MoveFunctionSignature) -> bool {
    let name = function.name.to_ascii_lowercase();
    let body = function.body.as_deref().unwrap_or("").to_ascii_lowercase();
    const PRIVILEGED_TERMS: &[&str] = &[
        "admin", "burn", "claim", "config", "create", "destroy", "fee", "mint", "owner", "pause",
        "set", "transfer", "treasury", "unpause", "update", "upgrade", "withdraw",
    ];
    const PRIVILEGED_BODY_TERMS: &[&str] = &[
        "balance::",
        "coin::",
        "dynamic_field",
        "event::emit",
        "object::new",
        "share_object",
        "transfer::",
        "tx_context::sender",
    ];

    PRIVILEGED_TERMS.iter().any(|term| name.contains(term))
        || PRIVILEGED_BODY_TERMS.iter().any(|term| body.contains(term))
}
