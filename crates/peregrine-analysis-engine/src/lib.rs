//! Shared orchestration for blockchain analysis plugins.

mod adapter_stage;
mod engine;
mod registry;

pub use engine::AnalysisEngine;
pub use registry::{AnalysisPluginRegistry, RegistryError};

#[cfg(test)]
mod dependency_tests {
    #[test]
    fn engine_manifest_has_no_chain_specific_dependencies() {
        let manifest = include_str!("../Cargo.toml");
        let Some(dependencies) = manifest
            .split("[dependencies]")
            .nth(1)
            .and_then(|section| section.split("[dev-dependencies]").next())
        else {
            panic!("analysis engine manifest has no dependencies section");
        };

        for forbidden in ["peregrine-sui", "sui-", "move-"] {
            assert!(
                !dependencies.contains(forbidden),
                "analysis engine dependency contains forbidden chain marker `{forbidden}`"
            );
        }
    }
}
