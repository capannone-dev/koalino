//! Makes SQLite erase deleted data instead of only marking its space as free.
//!
//! Every connection to the application database turns on `secure_delete`, because the setting
//! belongs to a connection, not to the file. Space freed before this existed still holds old
//! bytes, so `vacuum_once` rewrites an existing database one time to clear them.

use rusqlite::{Connection, params};

use super::sqlite_adapter::sqlite_error;
use crate::domain::errors::StorageError;

/// Names the one-time rewrite in `storage_migrations`.
const VACUUM_MIGRATION_NAME: &str = "secure_delete_vacuum";

/// Makes this connection overwrite deleted content with zeros.
///
/// Inputs: An open connection. Outputs: Success, or a storage error, so that a connection that
/// would leave deleted data readable is never used.
/// Side effects: Changes the connection's setting, and makes its deletions write more pages.
pub(super) fn enable(connection: &Connection) -> Result<(), StorageError> {
    let enabled: i64 = connection
        .query_row("PRAGMA secure_delete = ON", [], |row| row.get(0))
        .map_err(|error| sqlite_error("unable to enable secure deletion", error))?;
    if enabled == 1 {
        Ok(())
    } else {
        Err(StorageError::Rejected {
            reason: String::from("SQLite did not enable secure deletion"),
        })
    }
}

/// Rewrites the database once, clearing what earlier deletions left in free space.
///
/// Inputs: A connection to a file database, with no transaction open.
/// Outputs: Nothing. A failure, such as another process using the file, is logged and the rewrite
/// runs at the next start, because it must not stop the owner reaching their notes.
/// Side effects: May rewrite the whole file, needing up to its size again in free disk space, and
/// records that it ran.
pub(super) fn vacuum_once(connection: &Connection) {
    if let Err(error) = try_vacuum_once(connection) {
        tracing::warn!(error = %error, "the database could not be rewritten; retrying at next start");
    }
}

/// Runs the rewrite unless it is on record.
fn try_vacuum_once(connection: &Connection) -> rusqlite::Result<()> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS storage_migrations (name TEXT PRIMARY KEY NOT NULL);",
    )?;
    let already_done: bool = connection.query_row(
        "SELECT EXISTS(SELECT 1 FROM storage_migrations WHERE name = ?1)",
        params![VACUUM_MIGRATION_NAME],
        |row| row.get(0),
    )?;
    if already_done {
        return Ok(());
    }
    connection.execute_batch("VACUUM;")?;
    connection.execute(
        "INSERT INTO storage_migrations (name) VALUES (?1)",
        params![VACUUM_MIGRATION_NAME],
    )?;
    tracing::info!("Database rewritten to clear previously deleted data");
    Ok(())
}
