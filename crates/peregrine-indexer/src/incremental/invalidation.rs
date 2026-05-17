use serde::{Deserialize, Serialize};

use super::fingerprints::PackageFingerprints;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum InvalidationReason {
    MoveTomlChanged,
    PackageSummariesChanged,
    SourceChanged,
    DependencyMetadataChanged,
    CompilerVersionChanged,
    IndexerVersionChanged,
    ExtractionConfigChanged,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationPlan {
    pub reasons: Vec<InvalidationReason>,
    pub refresh_summary_pointers: bool,
    pub refresh_root_cards: bool,
    pub refresh_dependency_cards: bool,
    pub refresh_full_body_index: bool,
    pub refresh_dependency_resolution: bool,
    pub refresh_compiler_backed_facts: bool,
    pub tag_only_reindex: bool,
}

impl InvalidationPlan {
    pub fn is_clean(&self) -> bool {
        self.reasons.is_empty()
    }
}

pub fn compare_fingerprints(
    previous: Option<&PackageFingerprints>,
    current: &PackageFingerprints,
) -> InvalidationPlan {
    let Some(previous) = previous else {
        return InvalidationPlan {
            reasons: vec![
                InvalidationReason::MoveTomlChanged,
                InvalidationReason::PackageSummariesChanged,
                InvalidationReason::SourceChanged,
            ],
            refresh_summary_pointers: true,
            refresh_root_cards: true,
            refresh_dependency_cards: true,
            refresh_full_body_index: true,
            refresh_dependency_resolution: true,
            refresh_compiler_backed_facts: true,
            tag_only_reindex: false,
        };
    };

    let mut plan = InvalidationPlan::default();
    if previous.move_toml_hash != current.move_toml_hash {
        plan.reasons.push(InvalidationReason::MoveTomlChanged);
        plan.refresh_dependency_resolution = true;
        plan.refresh_summary_pointers = true;
    }
    if previous.package_summaries_hash != current.package_summaries_hash {
        plan.reasons
            .push(InvalidationReason::PackageSummariesChanged);
        plan.refresh_summary_pointers = true;
        plan.refresh_root_cards = true;
        plan.refresh_dependency_cards = true;
    }
    if previous.source_hash != current.source_hash {
        plan.reasons.push(InvalidationReason::SourceChanged);
        plan.refresh_full_body_index = true;
    }
    if previous.dependency_metadata_hash != current.dependency_metadata_hash {
        plan.reasons
            .push(InvalidationReason::DependencyMetadataChanged);
        plan.refresh_dependency_resolution = true;
        plan.refresh_dependency_cards = true;
    }
    if previous.compiler_version != current.compiler_version {
        plan.reasons
            .push(InvalidationReason::CompilerVersionChanged);
        plan.refresh_compiler_backed_facts = true;
    }
    if previous.indexer_version != current.indexer_version {
        plan.reasons.push(InvalidationReason::IndexerVersionChanged);
        plan.refresh_compiler_backed_facts = true;
        plan.refresh_root_cards = true;
    }
    if previous.extraction_config_hash != current.extraction_config_hash {
        plan.reasons
            .push(InvalidationReason::ExtractionConfigChanged);
        plan.tag_only_reindex = true;
    }
    plan
}
