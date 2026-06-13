mod bool_judgement;
mod common;
pub mod complexity;
mod infinite_loop;
mod precision_loss;
mod type_conversion;
mod unchecked_return;
mod unused_const;
mod unused_private_fun;
mod unused_struct;

use peregrine_types::analysis::{Rule, RuleSet, RuleSetMetadata, RuleSetProvider};

pub struct SuiRuleSetProvider;

impl RuleSetProvider for SuiRuleSetProvider {
    fn rule_sets(&self) -> Vec<Box<dyn RuleSet>> {
        vec![
            Box::new(SingleRuleSet::new(
                bool_judgement::RULE_ID,
                bool_judgement_rule,
            )),
            Box::new(SingleRuleSet::new(
                infinite_loop::RULE_ID,
                infinite_loop_rule,
            )),
            Box::new(SingleRuleSet::new(
                precision_loss::RULE_ID,
                precision_loss_rule,
            )),
            Box::new(SingleRuleSet::new(
                type_conversion::RULE_ID,
                type_conversion_rule,
            )),
            Box::new(SingleRuleSet::new(
                unchecked_return::RULE_ID,
                unchecked_return_rule,
            )),
            Box::new(SingleRuleSet::new(unused_const::RULE_ID, unused_const_rule)),
            Box::new(SingleRuleSet::new(
                unused_private_fun::RULE_ID,
                unused_private_function_rule,
            )),
            Box::new(SingleRuleSet::new(
                unused_struct::RULE_ID,
                unused_struct_rule,
            )),
        ]
    }
}

struct SingleRuleSet {
    id: &'static str,
    rule_factory: fn() -> Box<dyn Rule>,
}

impl SingleRuleSet {
    fn new(id: &'static str, rule_factory: fn() -> Box<dyn Rule>) -> Self {
        Self { id, rule_factory }
    }
}

impl RuleSet for SingleRuleSet {
    fn id(&self) -> &'static str {
        self.id
    }

    fn metadata(&self) -> RuleSetMetadata {
        let rule_metadata = (self.rule_factory)().metadata();

        RuleSetMetadata {
            id: self.id.to_string(),
            name: rule_metadata.name.clone(),
            description: rule_metadata.description.clone(),
            bundled: true,
            plugin_id: None,
            active: true,
            rules: vec![rule_metadata],
        }
    }

    fn rules(&self) -> Vec<Box<dyn Rule>> {
        vec![(self.rule_factory)()]
    }
}

fn bool_judgement_rule() -> Box<dyn Rule> {
    Box::new(bool_judgement::BoolJudgementRule)
}

fn infinite_loop_rule() -> Box<dyn Rule> {
    Box::new(infinite_loop::InfiniteLoopRule)
}

fn precision_loss_rule() -> Box<dyn Rule> {
    Box::new(precision_loss::PrecisionLossRule)
}

fn type_conversion_rule() -> Box<dyn Rule> {
    Box::new(type_conversion::TypeConversionRule)
}

fn unchecked_return_rule() -> Box<dyn Rule> {
    Box::new(unchecked_return::UncheckedReturnRule)
}

fn unused_const_rule() -> Box<dyn Rule> {
    Box::new(unused_const::UnusedConstRule)
}

fn unused_private_function_rule() -> Box<dyn Rule> {
    Box::new(unused_private_fun::UnusedPrivateFunctionRule)
}

fn unused_struct_rule() -> Box<dyn Rule> {
    Box::new(unused_struct::UnusedStructRule)
}
