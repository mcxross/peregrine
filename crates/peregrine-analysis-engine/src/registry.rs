use peregrine_analysis::{
    ChainAdapter, ChainId, DynamicAnalyzer, GraphBuilder, Scanner, StaticAnalyzer,
};
use std::{collections::BTreeMap, sync::Arc};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegistryError {
    pub message: String,
}

impl std::fmt::Display for RegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RegistryError {}

#[derive(Default)]
pub struct AnalysisPluginRegistry {
    adapters: BTreeMap<ChainId, Arc<dyn ChainAdapter>>,
    scanners: Vec<Arc<dyn Scanner>>,
    graph_builders: Vec<Arc<dyn GraphBuilder>>,
    static_analyzers: Vec<Arc<dyn StaticAnalyzer>>,
    dynamic_analyzers: Vec<Arc<dyn DynamicAnalyzer>>,
}

impl AnalysisPluginRegistry {
    pub fn register_adapter(
        &mut self,
        adapter: Arc<dyn ChainAdapter>,
    ) -> Result<(), RegistryError> {
        let descriptor = adapter.descriptor();
        if self.adapters.contains_key(&descriptor.chain) {
            return Err(RegistryError {
                message: format!(
                    "chain adapter for `{}` is already registered",
                    descriptor.chain.as_str()
                ),
            });
        }
        self.adapters.insert(descriptor.chain, adapter);
        Ok(())
    }

    pub fn register_scanner(&mut self, scanner: Arc<dyn Scanner>) {
        self.scanners.push(scanner);
        sort_plugins(&mut self.scanners, |plugin| plugin.descriptor());
    }

    pub fn register_graph_builder(&mut self, builder: Arc<dyn GraphBuilder>) {
        self.graph_builders.push(builder);
        sort_plugins(&mut self.graph_builders, |plugin| plugin.descriptor());
    }

    pub fn register_static_analyzer(&mut self, analyzer: Arc<dyn StaticAnalyzer>) {
        self.static_analyzers.push(analyzer);
        sort_plugins(&mut self.static_analyzers, |plugin| plugin.descriptor());
    }

    pub fn register_dynamic_analyzer(&mut self, analyzer: Arc<dyn DynamicAnalyzer>) {
        self.dynamic_analyzers.push(analyzer);
        sort_plugins(&mut self.dynamic_analyzers, |plugin| plugin.descriptor());
    }

    pub(crate) fn adapter(&self, chain: &ChainId) -> Option<Arc<dyn ChainAdapter>> {
        self.adapters.get(chain).cloned()
    }

    pub(crate) fn scanners(&self) -> &[Arc<dyn Scanner>] {
        &self.scanners
    }

    pub(crate) fn graph_builders(&self) -> &[Arc<dyn GraphBuilder>] {
        &self.graph_builders
    }

    pub(crate) fn static_analyzers(&self) -> &[Arc<dyn StaticAnalyzer>] {
        &self.static_analyzers
    }

    pub(crate) fn dynamic_analyzers(&self) -> &[Arc<dyn DynamicAnalyzer>] {
        &self.dynamic_analyzers
    }
}

fn sort_plugins<T>(
    plugins: &mut [Arc<T>],
    descriptor: impl Fn(&Arc<T>) -> peregrine_analysis::PluginDescriptor,
) where
    T: ?Sized,
{
    plugins.sort_by(|left, right| {
        let left = descriptor(left);
        let right = descriptor(right);
        right
            .priority
            .cmp(&left.priority)
            .then_with(|| left.id.cmp(&right.id))
    });
}
