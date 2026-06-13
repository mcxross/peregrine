use std::path::Path;

use crate::{
    core::{ContextBudget, IndexerResult},
    model::GraphView,
    storage::sqlite::SqliteIndexReader,
};

pub fn get_call_graph(
    db_path: impl AsRef<Path>,
    function_id: &str,
    depth: usize,
    budget: &ContextBudget,
) -> IndexerResult<GraphView> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_call_graph(function_id, depth, budget)?)
}
