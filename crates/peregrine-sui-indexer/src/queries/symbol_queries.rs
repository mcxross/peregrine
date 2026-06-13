use std::path::Path;

use crate::{
    core::{ContextBudget, IndexerResult, Operation},
    model::SymbolResult,
    storage::sqlite::SqliteIndexReader,
};

pub fn search_symbols(
    db_path: impl AsRef<Path>,
    package_id: &str,
    query: &str,
    budget: &ContextBudget,
) -> IndexerResult<Vec<SymbolResult>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.search_symbols(package_id, query, budget)?)
}

pub fn get_operations_by_tag(
    db_path: impl AsRef<Path>,
    package_id: &str,
    tag: &str,
    budget: &ContextBudget,
) -> IndexerResult<Vec<Operation>> {
    Ok(
        SqliteIndexReader::open(db_path.as_ref())?
            .get_operations_by_tag(package_id, tag, budget)?,
    )
}

pub fn get_functions_by_tag(
    db_path: impl AsRef<Path>,
    package_id: &str,
    tag: &str,
    budget: &ContextBudget,
) -> IndexerResult<Vec<SymbolResult>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_functions_by_tag(package_id, tag, budget)?)
}

pub fn get_public_entry_functions(
    db_path: impl AsRef<Path>,
    package_id: &str,
) -> IndexerResult<Vec<SymbolResult>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_public_entry_functions(package_id)?)
}
