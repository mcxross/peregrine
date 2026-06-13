fn operation_touches_type(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
    operations: &[&str],
) -> bool {
    let value_names = owned_or_constructed_value_names(body, signature, type_name, qualified_name);

    operation_call_snippets(body, operations)
        .iter()
        .any(|snippet| {
            body_constructs_type(snippet, type_name)
                || body_constructs_type(snippet, qualified_name)
                || value_names
                    .iter()
                    .any(|value_name| source_contains_identifier(snippet, value_name))
        })
}

fn delete_touches_type(body: &str, signature: &str, type_name: &str, qualified_name: &str) -> bool {
    let lower_body = body.to_ascii_lowercase();

    if !is_delete_signal(&lower_body) {
        return false;
    }

    let value_names = owned_or_constructed_value_names(body, signature, type_name, qualified_name);
    let destructures_type =
        body_destructures_type(body, type_name) || body_destructures_type(body, qualified_name);

    destructures_type
        || value_names
            .iter()
            .any(|value_name| source_contains_identifier(body, value_name))
}

fn owned_or_constructed_value_names(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    names.extend(owned_parameter_names(signature, type_name));
    names.extend(owned_parameter_names(signature, qualified_name));
    names.extend(constructed_value_names(body, type_name));
    names.extend(constructed_value_names(body, qualified_name));
    names
}

fn owned_parameter_names(signature: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(parameters) = function_parameters(signature) else {
        return names;
    };

    for parameter in split_top_level(parameters, ',') {
        let Some((name, parameter_type)) = parameter.split_once(':') else {
            continue;
        };
        let parameter_type = parameter_type.trim();

        if parameter_type.starts_with('&') || !type_reference_matches(parameter_type, type_name) {
            continue;
        }

        let name = name
            .split_whitespace()
            .last()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn constructed_value_names(body: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in body.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };

        if !left.contains("let") || !body_constructs_type(right, type_name) {
            continue;
        }

        let Some(raw_name) = left
            .split("let")
            .last()
            .and_then(|binding| binding.split(':').next())
        else {
            continue;
        };

        let name = raw_name
            .split_whitespace()
            .filter(|part| *part != "mut")
            .next_back()
            .unwrap_or("")
            .trim_matches(|character: char| !is_identifier_character(character));

        if !name.is_empty() {
            names.insert(name.to_string());
        }
    }

    names
}

fn body_destructures_type(body: &str, type_name: &str) -> bool {
    let short_name = type_name.rsplit("::").next().unwrap_or(type_name);

    body.contains(&format!("let {short_name} {{"))
        || body.contains(&format!("let {short_name}<"))
        || body.contains(&format!("let {type_name} {{"))
        || body.contains(&format!("let {type_name}<"))
}

fn operation_call_snippets(body: &str, operations: &[&str]) -> Vec<String> {
    let lower_body = body.to_ascii_lowercase();
    let mut snippets = Vec::new();

    for operation in operations {
        let operation = operation.to_ascii_lowercase();
        let mut search_start = 0;

        while let Some(relative_start) = lower_body[search_start..].find(&operation) {
            let start = search_start + relative_start;
            let end = lower_body[start..]
                .find(';')
                .map(|offset| start + offset)
                .unwrap_or(body.len());

            snippets.push(body[start..end].to_string());
            search_start = end.saturating_add(1);
        }
    }

    snippets
}
