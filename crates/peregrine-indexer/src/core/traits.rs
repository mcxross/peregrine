use std::path::Path;

use super::{FunctionId, OperationId, SourceSpan};
use crate::sui::model::{
    CompiledPackage, ExtractionContext, LoadedPackage, MaterializedSummaryContext, ProgramIndex,
    SummaryArtifacts, SummaryMaterializationRequest, SummaryPointerIndex,
};

pub type IndexerResult<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;

pub trait MoveIndexerAdapter {
    fn load_package(&self, root: &Path) -> IndexerResult<LoadedPackage>;
    fn compile_package(&self, package: LoadedPackage) -> IndexerResult<CompiledPackage>;
    fn discover_summaries(&self, package: &LoadedPackage) -> IndexerResult<SummaryArtifacts>;
    fn extract_summary_pointers(
        &self,
        artifacts: SummaryArtifacts,
    ) -> IndexerResult<SummaryPointerIndex>;
    fn materialize_summary_context(
        &self,
        request: SummaryMaterializationRequest,
    ) -> IndexerResult<MaterializedSummaryContext>;
    fn enrich_full_index(
        &self,
        compiled: CompiledPackage,
        summary: SummaryPointerIndex,
    ) -> IndexerResult<ProgramIndex>;
}

pub trait SourceMapper {
    fn span_for_function(&self, function_id: &FunctionId) -> Option<SourceSpan>;
    fn span_for_operation(&self, operation_id: &OperationId) -> Option<SourceSpan>;
}

pub trait Extractor<T> {
    fn extract(&self, ctx: &ExtractionContext) -> IndexerResult<T>;
}
