use std::path::Path;

use crate::{
    core::{ContextBudget, IndexerResult},
    model::ContextPack,
    storage::sqlite::SqliteIndexReader,
};

pub fn get_context_pack(
    db_path: impl AsRef<Path>,
    target_id: &str,
    budget: &ContextBudget,
) -> IndexerResult<ContextPack> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_context_pack(target_id, budget)?)
}
