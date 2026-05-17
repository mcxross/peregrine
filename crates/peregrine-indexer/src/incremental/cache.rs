use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};

use crate::core::IndexerResult;

use super::fingerprints::PackageFingerprints;

pub struct IncrementalCache {
    connection: Connection,
}

impl IncrementalCache {
    pub fn open(db_path: &Path) -> IndexerResult<Self> {
        let connection = Connection::open(db_path)?;
        connection.execute(
            "CREATE TABLE IF NOT EXISTS incremental_fingerprints (
              package_id TEXT PRIMARY KEY,
              fingerprints_json TEXT NOT NULL,
              updated_at INTEGER NOT NULL
            )",
            [],
        )?;
        Ok(Self { connection })
    }

    pub fn load(&self, package_id: &str) -> IndexerResult<Option<PackageFingerprints>> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT fingerprints_json FROM incremental_fingerprints WHERE package_id = ?1",
                [package_id],
                |row| row.get(0),
            )
            .optional()?;
        json.map(|json| serde_json::from_str(&json).map_err(Into::into))
            .transpose()
    }

    pub fn store(
        &self,
        package_id: &str,
        fingerprints: &PackageFingerprints,
        updated_at: i64,
    ) -> IndexerResult<()> {
        let json = serde_json::to_string(fingerprints)?;
        self.connection.execute(
            "INSERT INTO incremental_fingerprints (package_id, fingerprints_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(package_id) DO UPDATE SET fingerprints_json = excluded.fingerprints_json, updated_at = excluded.updated_at",
            params![package_id, json, updated_at],
        )?;
        Ok(())
    }
}
