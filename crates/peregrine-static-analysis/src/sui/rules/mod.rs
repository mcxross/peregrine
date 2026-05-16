mod bool_judgement;
mod common;
mod infinite_loop;
mod precision_loss;
mod type_conversion;
mod unchecked_return;
mod unused_const;
mod unused_private_fun;
mod unused_struct;

use peregrine_types::analysis::{Rule, RuleSet, RuleSetProvider};

pub use common::RULESET_ID;

pub struct SuiRuleSetProvider;

impl RuleSetProvider for SuiRuleSetProvider {
    fn rule_sets(&self) -> Vec<Box<dyn RuleSet>> {
        vec![Box::new(SuiRuleSet)]
    }
}

pub struct SuiRuleSet;

impl RuleSet for SuiRuleSet {
    fn id(&self) -> &'static str {
        RULESET_ID
    }

    fn rules(&self) -> Vec<Box<dyn Rule>> {
        vec![
            Box::new(bool_judgement::BoolJudgementRule),
            Box::new(infinite_loop::InfiniteLoopRule),
            Box::new(precision_loss::PrecisionLossRule),
            Box::new(type_conversion::TypeConversionRule),
            Box::new(unchecked_return::UncheckedReturnRule),
            Box::new(unused_const::UnusedConstRule),
            Box::new(unused_private_fun::UnusedPrivateFunctionRule),
            Box::new(unused_struct::UnusedStructRule),
        ]
    }
}
