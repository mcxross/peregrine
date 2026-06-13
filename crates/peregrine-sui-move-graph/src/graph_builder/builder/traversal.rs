impl GraphBuilder {
    fn traverse_sequence(
        &mut self,
        function: &mut FunctionContext,
        aliases: &AliasScope,
        sequence: &Sequence,
    ) {
        let mut scoped_aliases = aliases.clone();

        for use_decl in &sequence.0 {
            apply_use_decl(
                &mut scoped_aliases,
                use_decl,
                &function.module.address,
                &function.module.module_name,
            );
        }

        for item in &sequence.1 {
            match &item.value {
                SequenceItem_::Seq(exp) => self.traverse_exp(function, &scoped_aliases, exp),
                SequenceItem_::Declare(bindings, annotation) => {
                    self.traverse_bind_list(function, &scoped_aliases, &bindings.value);
                    if let Some(annotation) = annotation {
                        let uses = self.record_function_type_usage(
                            function,
                            annotation,
                            TypeUsageInput {
                                relationship: "annotation".to_string(),
                                function_name: Some(function.function_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                        self.bind_first_state_type(function, &bindings.value, &uses);
                    }
                }
                SequenceItem_::Bind(bindings, annotation, exp) => {
                    self.traverse_bind_list(function, &scoped_aliases, &bindings.value);
                    if let Some(annotation) = annotation {
                        let uses = self.record_function_type_usage(
                            function,
                            annotation,
                            TypeUsageInput {
                                relationship: "annotation".to_string(),
                                function_name: Some(function.function_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                        self.bind_first_state_type(function, &bindings.value, &uses);
                    } else if let Some(state_type_id) =
                        self.state_type_id_from_exp(function, &scoped_aliases, exp)
                    {
                        self.bind_state_type(function, &bindings.value, &state_type_id);
                    }
                    self.traverse_exp(function, &scoped_aliases, exp);
                }
            }
        }

        if let Some(exp) = sequence.3.as_ref() {
            self.traverse_exp(function, &scoped_aliases, exp);
        }
    }

    fn traverse_bind_list(
        &mut self,
        function: &mut FunctionContext,
        aliases: &AliasScope,
        bindings: &[Bind],
    ) {
        for bind in bindings {
            self.traverse_bind(function, aliases, bind);
        }
    }

    fn traverse_bind(&mut self, function: &mut FunctionContext, aliases: &AliasScope, bind: &Bind) {
        match &bind.value {
            Bind_::Var(_, _) => {}
            Bind_::Unpack(name, bindings) => {
                let span = source_span(
                    &function.module.source,
                    &function.module.file_path,
                    bind.loc.start() as usize,
                    bind.loc.end() as usize,
                );
                let destructured_type_id = self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "destructuring",
                    span.clone(),
                    None,
                );
                match bindings {
                    move_compiler::parser::ast::FieldBindings::Named(fields) => {
                        for field in fields {
                            if let move_compiler::parser::ast::Ellipsis::Binder((
                                field_name,
                                bind,
                            )) = field
                            {
                                if let Some(type_id) = destructured_type_id.as_deref() {
                                    self.record_state_field_access(
                                        function,
                                        type_id,
                                        &field_name.0.value.to_string(),
                                        "read",
                                        span.clone(),
                                        vec![format!(
                                            "Destructuring reads field `{}` from package state.",
                                            field_name.0.value
                                        )],
                                    );
                                }
                                self.traverse_bind(function, aliases, bind);
                            }
                        }
                    }
                    move_compiler::parser::ast::FieldBindings::Positional(fields) => {
                        for field in fields {
                            if let move_compiler::parser::ast::Ellipsis::Binder(bind) = field {
                                self.traverse_bind(function, aliases, bind);
                            }
                        }
                    }
                }
            }
        }
    }

    fn traverse_exp(&mut self, function: &mut FunctionContext, aliases: &AliasScope, exp: &Exp) {
        match &exp.value {
            Exp_::Value(_) | Exp_::Unit | Exp_::Spec(_) | Exp_::UnresolvedError => {}
            Exp_::Name(name) => self.record_name_state_access(
                function,
                name,
                "read",
                source_span(
                    &function.module.source,
                    &function.module.file_path,
                    exp.loc.start() as usize,
                    exp.loc.end() as usize,
                ),
            ),
            Exp_::Move(_, inner) => {
                self.traverse_state_target_exp(function, aliases, inner, "move")
            }
            Exp_::Copy(_, inner) => {
                self.traverse_state_target_exp(function, aliases, inner, "copy")
            }
            Exp_::Borrow(is_mutable, inner) => self.traverse_state_target_exp(
                function,
                aliases,
                inner,
                if *is_mutable {
                    "borrowMut"
                } else {
                    "borrowImm"
                },
            ),
            Exp_::Parens(inner)
            | Exp_::Dereference(inner)
            | Exp_::UnaryExp(_, inner)
            | Exp_::DotUnresolved(_, inner) => self.traverse_exp(function, aliases, inner),
            Exp_::Call(name, arguments) => {
                self.record_call(function, aliases, name, exp, &arguments.value, "direct");
            }
            Exp_::Pack(name, fields) => {
                let span = source_span(
                    &function.module.source,
                    &function.module.file_path,
                    exp.loc.start() as usize,
                    exp.loc.end() as usize,
                );
                let constructed_type_id = self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "construction",
                    span.clone(),
                    Some(function.function_name.clone()),
                );

                for (field_name, field_exp) in fields {
                    if let Some(type_id) = constructed_type_id.as_deref() {
                        self.record_state_field_access(
                            function,
                            type_id,
                            &field_name.0.value.to_string(),
                            "write",
                            span.clone(),
                            vec![format!(
                                "Construction writes field `{}` on package state.",
                                field_name.0.value
                            )],
                        );
                    }
                    self.traverse_exp(function, aliases, field_exp);
                }
            }
            Exp_::Vector(_, type_arguments, expressions) => {
                if let Some(type_arguments) = type_arguments {
                    for (index, type_argument) in type_arguments.iter().enumerate() {
                        self.record_function_type_usage(
                            function,
                            type_argument,
                            TypeUsageInput {
                                relationship: "vectorElement".to_string(),
                                function_name: Some(function.function_name.clone()),
                                type_argument_index: Some(index),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                for expression in &expressions.value {
                    self.traverse_exp(function, aliases, expression);
                }
            }
            Exp_::IfElse(condition, true_branch, false_branch) => {
                self.traverse_exp(function, aliases, condition);
                self.traverse_exp(function, aliases, true_branch);
                if let Some(false_branch) = false_branch {
                    self.traverse_exp(function, aliases, false_branch);
                }
            }
            Exp_::Match(subject, arms) => {
                self.traverse_exp(function, aliases, subject);
                for arm in &arms.value {
                    self.traverse_match_pattern(function, aliases, &arm.value.pattern);
                    if let Some(guard) = &arm.value.guard {
                        self.traverse_exp(function, aliases, guard);
                    }
                    self.traverse_exp(function, aliases, &arm.value.rhs);
                }
            }
            Exp_::While(condition, body) | Exp_::BinopExp(condition, _, body) => {
                self.traverse_exp(function, aliases, condition);
                self.traverse_exp(function, aliases, body);
            }
            Exp_::Loop(body) | Exp_::Labeled(_, body) => self.traverse_exp(function, aliases, body),
            Exp_::Block(sequence) => self.traverse_sequence(function, aliases, sequence),
            Exp_::Lambda(bindings, return_type, body) => {
                for (binding_list, annotation) in &bindings.value {
                    self.traverse_bind_list(function, aliases, &binding_list.value);
                    if let Some(annotation) = annotation {
                        self.record_function_type_usage(
                            function,
                            annotation,
                            TypeUsageInput {
                                relationship: "annotation".to_string(),
                                function_name: Some(function.function_name.clone()),
                                ..TypeUsageInput::default()
                            },
                        );
                    }
                }
                if let Some(return_type) = return_type {
                    self.record_function_type_usage(
                        function,
                        return_type,
                        TypeUsageInput {
                            relationship: "return".to_string(),
                            function_name: Some(function.function_name.clone()),
                            ..TypeUsageInput::default()
                        },
                    );
                }
                self.traverse_exp(function, aliases, body);
            }
            Exp_::Quant(_, bindings, triggers, condition, body) => {
                for binding in &bindings.value {
                    self.traverse_bind(function, aliases, &binding.value.0);
                    self.traverse_exp(function, aliases, &binding.value.1);
                }
                for trigger_group in triggers {
                    for trigger in trigger_group {
                        self.traverse_exp(function, aliases, trigger);
                    }
                }
                if let Some(condition) = condition {
                    self.traverse_exp(function, aliases, condition);
                }
                self.traverse_exp(function, aliases, body);
            }
            Exp_::ExpList(expressions) => {
                for expression in expressions {
                    self.traverse_exp(function, aliases, expression);
                }
            }
            Exp_::Assign(left, right) => {
                self.traverse_state_target_exp(function, aliases, left, "write");
                self.traverse_exp(function, aliases, right);
            }
            Exp_::Abort(value) | Exp_::Return(_, value) | Exp_::Break(_, value) => {
                if let Some(value) = value {
                    self.traverse_exp(function, aliases, value);
                }
            }
            Exp_::Continue(_) => {}
            Exp_::Dot(receiver, _, field) => {
                self.record_field_access_from_receiver(
                    function,
                    receiver,
                    &field.value.to_string(),
                    "read",
                    source_span(
                        &function.module.source,
                        &function.module.file_path,
                        exp.loc.start() as usize,
                        exp.loc.end() as usize,
                    ),
                );
                self.traverse_exp(function, aliases, receiver);
            }
            Exp_::DotCall(receiver, _, method, is_macro, type_arguments, arguments) => {
                self.traverse_exp(function, aliases, receiver);
                self.record_dot_call(
                    function,
                    aliases,
                    method,
                    is_macro.is_some(),
                    type_arguments.as_deref().unwrap_or(&[]),
                    &arguments.value,
                    exp,
                );
            }
            Exp_::Index(collection, indexes) => {
                self.traverse_exp(function, aliases, collection);
                for index in &indexes.value {
                    self.traverse_exp(function, aliases, index);
                }
            }
            Exp_::Cast(value, cast_type) => {
                self.traverse_exp(function, aliases, value);
                self.record_function_type_usage(
                    function,
                    cast_type,
                    TypeUsageInput {
                        relationship: "cast".to_string(),
                        function_name: Some(function.function_name.clone()),
                        ..TypeUsageInput::default()
                    },
                );
            }
            Exp_::Annotate(value, annotation) => {
                self.traverse_exp(function, aliases, value);
                self.record_function_type_usage(
                    function,
                    annotation,
                    TypeUsageInput {
                        relationship: "annotation".to_string(),
                        function_name: Some(function.function_name.clone()),
                        ..TypeUsageInput::default()
                    },
                );
            }
        }
    }

    fn traverse_match_pattern(
        &mut self,
        function: &mut FunctionContext,
        aliases: &AliasScope,
        pattern: &MatchPattern,
    ) {
        match &pattern.value {
            MatchPattern_::PositionalConstructor(name, fields) => {
                self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "destructuring",
                    source_span(
                        &function.module.source,
                        &function.module.file_path,
                        pattern.loc.start() as usize,
                        pattern.loc.end() as usize,
                    ),
                    Some(function.function_name.clone()),
                );

                for field in &fields.value {
                    if let move_compiler::parser::ast::Ellipsis::Binder(pattern) = field {
                        self.traverse_match_pattern(function, aliases, pattern);
                    }
                }
            }
            MatchPattern_::FieldConstructor(name, fields) => {
                let span = source_span(
                    &function.module.source,
                    &function.module.file_path,
                    pattern.loc.start() as usize,
                    pattern.loc.end() as usize,
                );
                let destructured_type_id = self.record_construct_or_destructure(
                    function,
                    aliases,
                    name,
                    "destructuring",
                    span.clone(),
                    Some(function.function_name.clone()),
                );

                for field in &fields.value {
                    if let move_compiler::parser::ast::Ellipsis::Binder((field_name, pattern)) =
                        field
                    {
                        if let Some(type_id) = destructured_type_id.as_deref() {
                            self.record_state_field_access(
                                function,
                                type_id,
                                &field_name.0.value.to_string(),
                                "read",
                                span.clone(),
                                vec![format!(
                                    "Match destructuring reads field `{}` from package state.",
                                    field_name.0.value
                                )],
                            );
                        }
                        self.traverse_match_pattern(function, aliases, pattern);
                    }
                }
            }
            MatchPattern_::Name(_, _) | MatchPattern_::Literal(_) => {}
            MatchPattern_::Or(left, right) => {
                self.traverse_match_pattern(function, aliases, left);
                self.traverse_match_pattern(function, aliases, right);
            }
            MatchPattern_::At(_, inner) => self.traverse_match_pattern(function, aliases, inner),
        }
    }

}
