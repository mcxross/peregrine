use rusqlite::Connection;

use super::schema::{CREATE_SCHEMA, SCHEMA_VERSION};

pub fn migrate(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(CREATE_SCHEMA)?;
    connection.execute(
        "INSERT OR REPLACE INTO schema_metadata (key, value) VALUES ('schema_version', ?1)",
        [SCHEMA_VERSION.to_string()],
    )?;
    Ok(())
}
