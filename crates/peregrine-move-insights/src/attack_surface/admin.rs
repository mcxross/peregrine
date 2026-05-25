use peregrine_move_model::MoveModule;
use std::collections::HashSet;

use super::{
    signature_helpers::{function_parameters_contain_type, privileged_function},
    types::AdminControlFinding,
};

pub(super) fn admin_control_findings(
    modules: &[MoveModule],
    capability_structs: &[String],
) -> Vec<AdminControlFinding> {
    let capability_names = capability_structs
        .iter()
        .map(|qualified| {
            qualified
                .rsplit("::")
                .next()
                .unwrap_or(qualified)
                .to_string()
        })
        .collect::<HashSet<_>>();
    let mut findings = Vec::new();

    for module in modules {
        for function in &module.functions {
            if !function.is_transaction_callable || !privileged_function(function) {
                continue;
            }

            let mut evidence = Vec::new();
            let mut guarding_types = Vec::new();

            for capability_name in &capability_names {
                if function_parameters_contain_type(&function.signature, capability_name) {
                    evidence.push(format!("requires capability parameter {capability_name}"));
                    guarding_types.push(capability_name.clone());
                }
            }

            let body = function.body.as_deref().unwrap_or("").to_ascii_lowercase();

            if body.contains("tx_context::sender") || body.contains("ctx.sender") {
                evidence.push("checks transaction sender".to_string());
            }

            if body.contains("assert!") {
                evidence
                    .push("contains assert-based authorization or invariant checks".to_string());
            }

            if evidence.is_empty() {
                evidence.push(
                    "transaction-callable privileged function without obvious guard".to_string(),
                );
            }

            findings.push(AdminControlFinding {
                function_name: function.name.clone(),
                module_name: module.name.clone(),
                qualified_name: format!("{}::{}", module.name, function.name),
                confidence: if guarding_types.is_empty() {
                    "medium"
                } else {
                    "high"
                }
                .to_string(),
                evidence,
                guarding_types,
            });
        }
    }

    findings.sort_by(|left, right| left.qualified_name.cmp(&right.qualified_name));
    findings
}
