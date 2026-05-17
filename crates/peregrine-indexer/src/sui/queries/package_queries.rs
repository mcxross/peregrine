use std::path::Path;

use crate::{core::IndexerResult, storage::sqlite::SqliteIndexReader, sui::model::PackageOverview};

pub fn get_package_overview(
    db_path: impl AsRef<Path>,
    package_id: &str,
) -> IndexerResult<PackageOverview> {
    Ok(SqliteIndexReader::open(db_path.as_ref())?.get_package_overview(package_id)?)
}
