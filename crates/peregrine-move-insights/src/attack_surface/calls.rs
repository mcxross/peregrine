fn external_call_findings(modules: &[MoveModule]) -> Vec<ExternalCallFinding> {
    let local_modules = modules
        .iter()
        .map(|module| module.name.as_str())
        .collect::<HashSet<_>>();
    let mut findings = Vec::new();
    let mut seen = HashSet::new();

    for module in modules {
        for function in &module.functions {
            let Some(body) = function.body.as_deref() else {
                continue;
            };

            for target in call_targets(body) {
                let Some((target_module, target_function)) = target.rsplit_once("::") else {
                    continue;
                };

                if local_modules.contains(target_module) || target_module == module.name {
                    continue;
                }

                let key = format!("{}::{}->{target}", module.name, function.name);

                if !seen.insert(key) {
                    continue;
                }

                findings.push(ExternalCallFinding {
                    caller_module: module.name.clone(),
                    caller_function: function.name.clone(),
                    target_module: target_module.to_string(),
                    target_function: target_function.to_string(),
                    target,
                });
            }
        }
    }

    findings.sort_by(|left, right| {
        left.caller_module
            .cmp(&right.caller_module)
            .then_with(|| left.caller_function.cmp(&right.caller_function))
            .then_with(|| left.target.cmp(&right.target))
    });
    findings
}

fn public_package_relationships(modules: &[MoveModule]) -> Vec<PublicPackageRelationship> {
    let public_package_functions = modules
        .iter()
        .flat_map(|module| {
            module
                .functions
                .iter()
                .filter(|function| function.visibility == "public(package)")
                .map(|function| (module.name.as_str(), function.name.as_str()))
        })
        .collect::<Vec<_>>();
    let mut relationships = Vec::new();
    let mut seen = HashSet::new();

    for caller_module in modules {
        for caller in &caller_module.functions {
            let Some(body) = caller.body.as_deref() else {
                continue;
            };

            for (target_module, target_function) in &public_package_functions {
                let qualified_call = format!("{target_module}::{target_function}");
                let same_module_call = caller_module.name == *target_module
                    && body.contains(&format!("{target_function}("));

                if !body.contains(&qualified_call) && !same_module_call {
                    continue;
                }

                let key = format!(
                    "{}::{}->{}::{}",
                    caller_module.name, caller.name, target_module, target_function
                );

                if !seen.insert(key) {
                    continue;
                }

                relationships.push(PublicPackageRelationship {
                    source_module: caller_module.name.clone(),
                    source_function: caller.name.clone(),
                    target_module: (*target_module).to_string(),
                    target_function: (*target_function).to_string(),
                });
            }
        }
    }

    relationships
}

fn call_targets(source: &str) -> Vec<String> {
    source
        .split(|character: char| {
            character.is_whitespace() || matches!(character, '(' | ')' | ',' | ';' | '{' | '}')
        })
        .filter(|token| token.contains("::"))
        .filter_map(|token| {
            let target = token
                .trim_matches(|character: char| {
                    matches!(character, '&' | '*' | '<' | '>' | ':' | ',' | ';' | '=')
                })
                .trim_end_matches('!');

            if target.matches("::").count() == 1 {
                Some(target.to_string())
            } else {
                None
            }
        })
        .collect()
}
