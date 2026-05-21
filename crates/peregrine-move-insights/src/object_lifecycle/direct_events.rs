fn function_lifecycle_events(
    object: &ObjectCandidate<'_>,
    module: &MoveModule,
    function: &MoveFunctionSignature,
) -> Vec<(String, String)> {
    let mut events = Vec::new();
    let Some(body) = function.body.as_deref() else {
        return events;
    };

    let type_name = &object.move_struct.name;
    let qualified_name = &object.qualified_name;
    let lower_body = body.to_ascii_lowercase();
    let constructs_type =
        body_constructs_type(body, type_name) || body_constructs_type(body, qualified_name);
    let returns_type = function_returns_type(&function.signature, type_name)
        || function_returns_type(&function.signature, qualified_name);
    let function_label = format!("{}::{}", module.name, function.name);

    if constructs_type && (lower_body.contains("object::new") || returns_type) {
        events.push((
            "created".to_string(),
            format!("{type_name} constructed in {function_label}"),
        ));
    }

    if function_mutably_touches_type(&function.signature, type_name)
        || function_mutably_touches_type(&function.signature, qualified_name)
    {
        events.push((
            "mutated".to_string(),
            format!("{function_label} takes &mut {type_name}"),
        ));
    }

    if borrowed_identity_mutates_related_state(body, &function.signature, type_name, qualified_name)
    {
        events.push((
            "mutated".to_string(),
            format!("{function_label} mutates state keyed by {type_name} identity"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::transfer",
            "transfer::public_transfer",
            "public_transfer",
        ],
    ) {
        events.push((
            "transferred".to_string(),
            format!("ownership transferred in {function_label}"),
        ));
        events.push((
            "owned".to_string(),
            format!("address-owned object path in {function_label}"),
        ));
    }

    if function.is_transaction_callable && returns_type {
        events.push((
            "transferred".to_string(),
            format!("returned to transaction caller from {function_label}"),
        ));
        events.push((
            "owned".to_string(),
            format!("returned from transaction-callable {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::share_object",
            "transfer::public_share_object",
            "share_object",
        ],
    ) {
        events.push((
            "shared".to_string(),
            format!("shared via transfer::share_object in {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::freeze_object",
            "transfer::public_freeze_object",
            "freeze_object",
        ],
    ) {
        events.push((
            "immutable".to_string(),
            format!("frozen via transfer::freeze_object in {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "transfer::party_transfer",
            "transfer::public_party_transfer",
            "party_transfer",
            "party::",
        ],
    ) {
        events.push((
            "party".to_string(),
            format!("moved through party ownership API in {function_label}"),
        ));
    }

    if delete_touches_type(body, &function.signature, type_name, qualified_name) {
        events.push((
            "deleted".to_string(),
            format!("delete signal observed in {function_label}"),
        ));
    }

    if operation_touches_type(
        body,
        &function.signature,
        type_name,
        qualified_name,
        &[
            "dynamic_field::add",
            "dynamic_object_field::add",
            "table::add",
            "bag::add",
        ],
    ) {
        events.push((
            "wrapped".to_string(),
            format!("stored through dynamic object storage in {function_label}"),
        ));
    }

    events
}

