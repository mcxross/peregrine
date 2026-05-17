use std::path::Path;

use crate::{
    core::{IndexerResult, MoveIndexerAdapter, PackageStatus},
    sui::{
        extractors::bytecode_extractor,
        model::{
            CompiledPackage, LoadedPackage, MaterializedSummaryContext, ProgramIndex,
            SummaryArtifacts, SummaryMaterializationRequest, SummaryPointerIndex,
        },
        package_loader, source_map, summary_loader,
    },
};

#[derive(Clone, Debug)]
pub struct SuiIndexerAdapter {
    debug_store_raw_summary_json: bool,
}

impl SuiIndexerAdapter {
    pub fn new(debug_store_raw_summary_json: bool) -> Self {
        Self {
            debug_store_raw_summary_json,
        }
    }
}

impl MoveIndexerAdapter for SuiIndexerAdapter {
    fn load_package(&self, root: &Path) -> IndexerResult<LoadedPackage> {
        package_loader::load_package(root)
    }

    fn compile_package(&self, package: LoadedPackage) -> IndexerResult<CompiledPackage> {
        package_loader::compile_package(package)
    }

    fn discover_summaries(&self, package: &LoadedPackage) -> IndexerResult<SummaryArtifacts> {
        package_loader::discover_summaries(package)
    }

    fn extract_summary_pointers(
        &self,
        artifacts: SummaryArtifacts,
    ) -> IndexerResult<SummaryPointerIndex> {
        summary_loader::extract_summary_pointers(artifacts, self.debug_store_raw_summary_json)
    }

    fn materialize_summary_context(
        &self,
        request: SummaryMaterializationRequest,
    ) -> IndexerResult<MaterializedSummaryContext> {
        summary_loader::materialize_summary_context(request)
    }

    fn enrich_full_index(
        &self,
        compiled: CompiledPackage,
        mut summary: SummaryPointerIndex,
    ) -> IndexerResult<ProgramIndex> {
        for mut diagnostic in compiled.diagnostics {
            diagnostic.package_id = summary.program_index.package.id.clone();
            summary.program_index.diagnostics.push(diagnostic);
        }

        if !summary.program_index.diagnostics.is_empty()
            && summary.program_index.package.status == PackageStatus::Indexed
        {
            summary.program_index.package.status = PackageStatus::PartialWithDiagnostics;
        }

        if let Some(build_root) = compiled.build_root {
            source_map::enrich_source_spans_from_sources(
                &mut summary.program_index,
                &compiled.loaded.root,
            );
            bytecode_extractor::enrich_program_from_build(&mut summary.program_index, &build_root);
        } else {
            source_map::enrich_source_spans_from_sources(
                &mut summary.program_index,
                &compiled.loaded.root,
            );
        }

        Ok(summary.program_index)
    }
}
