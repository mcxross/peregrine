use std::path::Path;

use crate::{
    core::{ContextBudget, IndexerResult, Operation},
    model::FunctionContext,
    storage::sqlite::SqliteIndexReader,
};

pub fn get_function_context(
    db_path: impl AsRef<Path>,
    function_id: &str,
    budget: &ContextBudget,
) -> IndexerResult<FunctionContext> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_context(function_id, budget)?)
}

pub fn get_function_body(
    db_path: impl AsRef<Path>,
    function_id: &str,
    budget: &ContextBudget,
) -> IndexerResult<FunctionContext> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_body(function_id, budget)?)
}

pub fn get_function_operations(
    db_path: impl AsRef<Path>,
    function_id: &str,
    budget: &ContextBudget,
) -> IndexerResult<Vec<Operation>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_operations(function_id, budget)?)
}

pub fn get_function_callers(
    db_path: impl AsRef<Path>,
    function_id: &str,
    budget: &ContextBudget,
) -> IndexerResult<Vec<String>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_callers(function_id, budget)?)
}

pub fn get_function_callees(
    db_path: impl AsRef<Path>,
    function_id: &str,
    budget: &ContextBudget,
) -> IndexerResult<Vec<String>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_callees(function_id, budget)?)
}

pub fn get_reachable_callees(
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
    db_path: impl AsRef<Path>,
    function_id: &str,
) -> IndexerResult<Vec<String>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_field_reads(function_id)?)
}

pub fn get_function_field_writes(
    db_path: impl AsRef<Path>,
    function_id: &str,
) -> IndexerResult<Vec<String>> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_function_field_writes(function_id)?)
}
