use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{
    config::IndexerConfig,
    core::{hash_str, ContextBudget, IndexerResult, MoveIndexerAdapter, Operation},
    incremental::{compare_fingerprints, fingerprint_package, IncrementalCache},
    storage::sqlite::{SqliteIndexReader, SqliteIndexWriter},
    sui::{
        index_health::harden_program_index,
        index_layers::summarize_program_layers,
        model::{
            ContextPack, FunctionContext, GraphView, IndexReport, MaterializedSummaryContext,
            ModuleContext, ModuleSummaryCard, PackageOverview, SymbolResult, TypeContext,
        },
        package_loader, SuiIndexerAdapter,
    },
};

pub struct SuiMoveIndexer {
    config: IndexerConfig,
    adapter: SuiIndexerAdapter,
}

impl SuiMoveIndexer {
    pub fn new(config: IndexerConfig) -> Self {
        let adapter = SuiIndexerAdapter::new(config.debug_store_raw_summary_json);
        Self { config, adapter }
    }

    pub fn index_package(
        &self,
        root: impl AsRef<Path>,
        run_id: String,
    ) -> IndexerResult<IndexReport> {
        let loaded = self.adapter.load_package(root.as_ref())?;
        let db_path = self.db_path_for_root(&loaded.root)?;
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut fingerprints = fingerprint_package(&loaded.root, env!("CARGO_PKG_VERSION"))?;
        fingerprints.extraction_config_hash = Some(hash_str(
            &serde_json::json!({
                "debugStoreRawSummaryJson": self.config.debug_store_raw_summary_json,
                "enrichFullMode": self.config.enrich_full_mode,
            })
            .to_string(),
        ));

        let artifacts = self.adapter.discover_summaries(&loaded)?;
        let summary = self.adapter.extract_summary_pointers(artifacts)?;
        let compiled = self.adapter.compile_package(loaded)?;
        let mut program = if self.config.enrich_full_mode {
            self.adapter.enrich_full_index(compiled, summary)?
        } else {
            summary.program_index
        };
        if program.package.compiler_version.is_none() {
            program.package.compiler_version = package_loader::local_sui_cli_version();
        }
        fingerprints.compiler_version = program.package.compiler_version.clone();

        let incremental_cache = IncrementalCache::open(&db_path)?;
        let previous_fingerprints = incremental_cache.load(&program.package.id)?;
        let invalidation = compare_fingerprints(previous_fingerprints.as_ref(), &fingerprints);
        let index_health = harden_program_index(
            &mut program,
            &fingerprints,
            previous_fingerprints.is_some(),
            &invalidation,
            self.config.enrich_full_mode,
        );

        let mut writer = SqliteIndexWriter::open(&db_path)?;
        writer.write_program_index(&program)?;
        incremental_cache.store(
            &program.package.id,
            &fingerprints,
            program.package.indexed_at,
        )?;

        Ok(IndexReport {
            run_id,
            package_id: program.package.id.clone(),
            package_name: program.package.name.clone(),
            db_path: db_path.to_string_lossy().into_owned(),
            status: format!("{:?}", program.package.status),
            index_health: Some(index_health),
            index_layers: summarize_program_layers(&program),
            summary_artifact_count: program.summary_artifacts.len(),
            module_count: program.modules.len(),
            function_count: program.functions.len(),
            type_count: program.types.len(),
            operation_count: program.operations.len(),
            diagnostic_count: program.diagnostics.len(),
        })
    }

    pub fn reindex_package(
        &self,
        db_path: impl AsRef<Path>,
        package_id: &str,
        run_id: String,
    ) -> IndexerResult<IndexReport> {
        let reader = SqliteIndexReader::open(db_path.as_ref())?;
        let root = reader
            .package_root_path(package_id)?
            .ok_or_else(|| format!("Package `{package_id}` not found"))?;
        self.index_package(root, run_id)
    }

    pub fn get_package_overview(
        &self,
        db_path: impl AsRef<Path>,
        package_id: &str,
    ) -> IndexerResult<PackageOverview> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_package_overview(package_id)?)
    }

    pub fn get_module_context(
        &self,
        db_path: impl AsRef<Path>,
        module_id: &str,
    ) -> IndexerResult<ModuleContext> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_module_context(module_id)?)
    }

    pub fn get_type_context(
        &self,
        db_path: impl AsRef<Path>,
        type_id: &str,
    ) -> IndexerResult<TypeContext> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_type_context(type_id)?)
    }

    pub fn get_function_context(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<FunctionContext> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_context(function_id, budget)?)
    }

    pub fn get_function_body(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<FunctionContext> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_body(function_id, budget)?)
    }

    pub fn get_function_operations(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<Vec<Operation>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?
            .get_function_operations(function_id, budget)?)
    }

    pub fn get_function_callers(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<Vec<String>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_callers(function_id, budget)?)
    }

    pub fn get_function_callees(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<Vec<String>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_callees(function_id, budget)?)
    }

    pub fn get_reachable_callees(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
        depth: usize,
        budget: &ContextBudget,
    ) -> IndexerResult<Vec<String>> {
        Ok(
            SqliteIndexReader::open(db_path.as_ref())?.get_reachable_callees(
                function_id,
                depth,
                budget,
            )?,
        )
    }

    pub fn get_function_field_reads(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
    ) -> IndexerResult<Vec<String>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_field_reads(function_id)?)
    }

    pub fn get_function_field_writes(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
    ) -> IndexerResult<Vec<String>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_field_writes(function_id)?)
    }

    pub fn get_call_graph(
        &self,
        db_path: impl AsRef<Path>,
        function_id: &str,
        depth: usize,
        budget: &ContextBudget,
    ) -> IndexerResult<GraphView> {
        Ok(
            SqliteIndexReader::open(db_path.as_ref())?.get_call_graph(
                function_id,
                depth,
                budget,
            )?,
        )
    }

    pub fn search_symbols(
        &self,
        db_path: impl AsRef<Path>,
        package_id: &str,
        query: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<Vec<SymbolResult>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.search_symbols(package_id, query, budget)?)
    }

    pub fn get_operations_by_tag(
        &self,
        db_path: impl AsRef<Path>,
        package_id: &str,
        tag: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<Vec<Operation>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?
            .get_operations_by_tag(package_id, tag, budget)?)
    }

    pub fn get_functions_by_tag(
        &self,
        db_path: impl AsRef<Path>,
        package_id: &str,
        tag: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<Vec<SymbolResult>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?
            .get_functions_by_tag(package_id, tag, budget)?)
    }

    pub fn get_public_entry_functions(
        &self,
        db_path: impl AsRef<Path>,
        package_id: &str,
    ) -> IndexerResult<Vec<SymbolResult>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_public_entry_functions(package_id)?)
    }

    pub fn get_context_pack(
        &self,
        db_path: impl AsRef<Path>,
        target_id: &str,
        budget: &ContextBudget,
    ) -> IndexerResult<ContextPack> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.get_context_pack(target_id, budget)?)
    }

    pub fn get_diagnostics(
        &self,
        db_path: impl AsRef<Path>,
        package_id: &str,
    ) -> IndexerResult<Vec<crate::core::Diagnostic>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?.diagnostics_for_package(package_id)?)
    }

    pub fn materialize_summary_module(
        &self,
        db_path: impl AsRef<Path>,
        package_alias: &str,
        module_name: &str,
        budget: ContextBudget,
    ) -> IndexerResult<ModuleSummaryCard> {
        let context =
            self.materialize_summary(db_path, package_alias, module_name, None, budget)?;
        Ok(context.card)
    }

    pub fn materialize_summary_symbol(
        &self,
        db_path: impl AsRef<Path>,
        package_alias: &str,
        module_name: &str,
        symbol_name: &str,
        budget: ContextBudget,
    ) -> IndexerResult<ModuleSummaryCard> {
        let context = self.materialize_summary(
            db_path,
            package_alias,
            module_name,
            Some(symbol_name.to_string()),
            budget,
        )?;
        Ok(context.card)
    }

    pub fn get_summary_artifact_pointer(
        &self,
        db_path: impl AsRef<Path>,
        package_alias: &str,
        module_name: &str,
    ) -> IndexerResult<Option<ModuleSummaryCard>> {
        Ok(SqliteIndexReader::open(db_path.as_ref())?
            .get_summary_artifact_pointer(package_alias, module_name)?)
    }

    fn materialize_summary(
        &self,
        db_path: impl AsRef<Path>,
        package_alias: &str,
        module_name: &str,
        symbol_name: Option<String>,
        budget: ContextBudget,
    ) -> IndexerResult<MaterializedSummaryContext> {
        self.adapter
            .materialize_summary_context(crate::sui::model::SummaryMaterializationRequest {
                db_path: db_path.as_ref().to_path_buf(),
                package_alias: package_alias.to_string(),
                module_name: module_name.to_string(),
                symbol_name,
                budget,
            })
    }

    fn db_path_for_root(&self, root: &Path) -> IndexerResult<PathBuf> {
        Ok(self
            .config
            .db_path
            .clone()
            .unwrap_or_else(|| root.join(".peregrine").join("index.sqlite")))
    }
}
