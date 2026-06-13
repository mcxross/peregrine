use std::path::Path;

use crate::{
    core::IndexerResult,
    model::{ModuleContext, TypeContext},
    storage::sqlite::SqliteIndexReader,
};

pub fn get_module_context(
    db_path: impl AsRef<Path>,
    module_id: &str,
) -> IndexerResult<ModuleContext> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_module_context(module_id)?)
}

pub fn get_type_context(db_path: impl AsRef<Path>, type_id: &str) -> IndexerResult<TypeContext> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_type_context(type_id)?)
}
