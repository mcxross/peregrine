fn is_test_module(module: &MoveModule) -> bool {
    module
        .file_path
        .split('/')
        .any(|segment| segment == "tests")
        || has_test_attribute(&module.attributes)
}

fn has_test_attribute(attributes: &[String]) -> bool {
    attributes.iter().any(|attribute| {
        let attribute = attribute.to_ascii_lowercase();
        attribute.contains("test")
            || attribute.contains("test_only")
            || attribute.contains("random_test")
            || attribute.contains("expected_failure")
    })
}

fn struct_has_ability(move_struct: &MoveStructSignature, ability: &str) -> bool {
    move_struct
        .abilities
        .iter()
        .any(|candidate| candidate == ability)
}

fn function_mutably_touches_type(signature: &str, type_name: &str) -> bool {
    let Some(parameters) = function_parameters(signature) else {
        return false;
    };

    split_top_level(parameters, ',')
        .into_iter()
        .any(|parameter| {
            let Some((_, parameter_type)) = parameter.split_once(':') else {
                return false;
            };
            let parameter_type = parameter_type.trim();

            parameter_type.starts_with("&mut") && type_reference_matches(parameter_type, type_name)
        })
}

fn borrowed_identity_mutates_related_state(
    body: &str,
    signature: &str,
    type_name: &str,
    qualified_name: &str,
) -> bool {
    let body_block = function_body_block(body);
    let borrowed_names = borrowed_parameter_names(signature, type_name)
        .into_iter()
        .chain(borrowed_parameter_names(signature, qualified_name))
        .collect::<BTreeSet<_>>();

    if borrowed_names.is_empty() {
        return false;
    }

    let identity_names = object_identity_names(body_block, &borrowed_names);

    if identity_names.is_empty() {
        return false;
    }

    body_block.split(';').any(|statement| {
        identity_names
            .iter()
            .any(|identity| source_contains_identifier(statement, identity))
            && statement_has_mutation_signal(statement)
    })
}

fn function_body_block(function_source: &str) -> &str {
    let Some(start) = function_source.find('{') else {
        return function_source;
    };
    let Some(end) = function_source.rfind('}') else {
        return &function_source[start + 1..];
    };

    if start < end {
        &function_source[start + 1..end]
    } else {
        function_source
    }
}

fn borrowed_parameter_names(signature: &str, type_name: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let Some(parameters) = function_parameters(signature) else {
        return names;
    };

    for parameter in split_top_level(parameters, ',') {
        let Some((name, parameter_type)) = parameter.split_once(':') else {
            continue;
        };
        let parameter_type = parameter_type.trim();

        if !parameter_type.starts_with('&')
            || parameter_type.starts_with("&mut")
            || !type_reference_matches(parameter_type, type_name)
        {
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

fn object_identity_names(body: &str, object_names: &BTreeSet<String>) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for statement in body.split(';') {
        let Some((left, right)) = statement.split_once('=') else {
            continue;
        };

        if !left.contains("let") {
            continue;
        }

        let derives_identity = object_names.iter().any(|object_name| {
            right.contains(&format!("object::id({object_name})"))
                || right.contains(&format!("object::id(&{object_name})"))
                || right.contains(&format!("{object_name}.id"))
        });

        if !derives_identity {
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

fn statement_has_mutation_signal(statement: &str) -> bool {
    let statement = statement.to_ascii_lowercase();
    const MUTATION_SIGNALS: &[&str] = &[
        "&mut",
        "_mut",
        "borrow_mut",
        "set_",
        "add_",
        "remove",
        "delete",
        "destroy",
        "insert",
        "push_back",
        ".add(",
        "::add(",
        ".remove(",
        "::remove(",
        "deposit",
        "withdraw",
        "supply",
        "split(",
        "decrease",
        "increase",
        "increment",
        "decrement",
        "latch",
        "refresh",
    ];

    MUTATION_SIGNALS
        .iter()
        .any(|signal| statement.contains(signal))
}

