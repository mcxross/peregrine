use crate::{AnalysisContext, Finding, RuleConfig, RuleMetric};

pub trait Rule: Send + Sync {
    fn id(&self) -> &'static str;
    fn analyze(&self, context: &AnalysisContext, config: &RuleConfig) -> RuleOutcome;
}

pub trait RuleSet: Send + Sync {
    fn id(&self) -> &'static str;
    fn rules(&self) -> Vec<Box<dyn Rule>>;
}

pub trait RuleSetProvider: Send + Sync {
    fn rule_sets(&self) -> Vec<Box<dyn RuleSet>>;
}

#[derive(Debug, Default)]
pub struct RuleOutcome {
    pub findings: Vec<Finding>,
    pub metrics: Vec<RuleMetric>,
}
