//! Keeps chats with notes (#97) in SQLite, sealed.
//!
//! Table `chat_messages`: one row per message, keyed by the chat (`ChatScope::key`) and its
//! position. The role and time are in clear; the text, what an answer included and left out, and
//! the connection's label are one sealed JSON value, bound to the chat and position, so a value
//! moved onto another row does not open. Deleting a meeting or a project deletes its chat
//! (`sqlite_deletions`, `sqlite_projects`). Nothing here logs messages.

use chrono::{DateTime, SecondsFormat, Utc};
use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::{SqliteNoteRepository, WriteTransaction, sqlite_error};
use crate::{
    domain::{
        chat::{ChatMessage, ChatRole, ChatScope, ChatSource, MAX_CHAT_MESSAGES},
        errors::StorageError,
    },
    ports::chat::ChatHistoryRepository,
};

/// Creates the chat table.
///
/// Side effects: May create a table and an index.
pub(super) fn initialize_chat_schema(connection: &Connection) -> Result<(), StorageError> {
    connection
        .execute_batch(
            "CREATE TABLE IF NOT EXISTS chat_messages (
                chat_key TEXT NOT NULL,
                position INTEGER NOT NULL,
                role TEXT NOT NULL,
                created_at_utc TEXT NOT NULL,
                sealed_body BLOB NOT NULL,
                PRIMARY KEY (chat_key, position)
            );",
        )
        .map_err(|error| sqlite_error("unable to create the chat table", error))
}

/// The sealed part of a message.
#[derive(Serialize, Deserialize)]
struct SealedBody {
    text: String,
    #[serde(default)]
    included: Vec<String>,
    #[serde(default)]
    omitted: Vec<String>,
    #[serde(default)]
    connection_label: Option<String>,
    /// The places an answer cites (#110); absent in messages stored before.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sources: Vec<ChatSource>,
}

/// The context binding a message to its chat and position.
fn context(chat_key: &str, position: i64) -> String {
    format!("chat-message:v1:{chat_key}:{position}")
}

impl ChatHistoryRepository for SqliteNoteRepository {
    fn chat_history(&self, scope: &ChatScope) -> Result<Vec<ChatMessage>, StorageError> {
        let chat_key = scope.key();
        let mut statement = self
            .connection
            .prepare(
                "SELECT position, role, created_at_utc, sealed_body FROM chat_messages
                 WHERE chat_key = ?1 ORDER BY position",
            )
            .map_err(|error| sqlite_error("unable to read a chat", error))?;
        let rows = statement
            .query_map(params![chat_key], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Vec<u8>>(3)?,
                ))
            })
            .map_err(|error| sqlite_error("unable to read a chat", error))?;
        let mut messages = Vec::new();
        for row in rows {
            let (position, role, created_at, sealed) =
                row.map_err(|error| sqlite_error("unable to read a chat message", error))?;
            let clear = self
                .embedding_cipher
                .open_secret(&context(&chat_key, position), &sealed)
                .map_err(|error| StorageError::Rejected {
                    reason: format!("unable to decrypt a chat message: {error}"),
                })?;
            let body: SealedBody =
                serde_json::from_slice(&clear).map_err(|_| StorageError::Rejected {
                    reason: String::from("a chat message is malformed"),
                })?;
            messages.push(ChatMessage {
                role: ChatRole::parse(&role).unwrap_or(ChatRole::User),
                text: body.text,
                created_at: DateTime::parse_from_rfc3339(&created_at)
                    .map(|at| at.with_timezone(&Utc))
                    .map_err(|_| StorageError::Rejected {
                        reason: String::from("a chat message has a malformed time"),
                    })?,
                included: body.included,
                omitted: body.omitted,
                connection_label: body.connection_label,
                sources: body.sources,
            });
        }
        Ok(messages)
    }

    fn append_chat_messages(
        &self,
        scope: &ChatScope,
        messages: &[ChatMessage],
    ) -> Result<(), StorageError> {
        let chat_key = scope.key();
        let transaction = self
            .connection
            .write_transaction()
            .map_err(|error| sqlite_error("unable to begin a chat write", error))?;
        let mut position: i64 = transaction
            .query_row(
                "SELECT COALESCE(MAX(position), -1) FROM chat_messages WHERE chat_key = ?1",
                params![chat_key],
                |row| row.get(0),
            )
            .map_err(|error| sqlite_error("unable to read a chat", error))?;
        for message in messages {
            position += 1;
            let body = serde_json::to_vec(&SealedBody {
                text: message.text.clone(),
                included: message.included.clone(),
                omitted: message.omitted.clone(),
                connection_label: message.connection_label.clone(),
                sources: message.sources.clone(),
            })
            .map_err(|_| StorageError::Rejected {
                reason: String::from("a chat message could not be encoded"),
            })?;
            let sealed = self
                .embedding_cipher
                .seal_secret(&context(&chat_key, position), &body)
                .map_err(|error| StorageError::Rejected {
                    reason: format!("unable to encrypt a chat message: {error}"),
                })?;
            transaction
                .execute(
                    "INSERT INTO chat_messages (chat_key, position, role, created_at_utc, sealed_body)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                    params![
                        chat_key,
                        position,
                        message.role.as_str(),
                        time_text(message.created_at),
                        sealed
                    ],
                )
                .map_err(|error| sqlite_error("unable to write a chat message", error))?;
        }
        transaction
            .execute(
                "DELETE FROM chat_messages WHERE chat_key = ?1 AND position <= ?2",
                params![chat_key, position - MAX_CHAT_MESSAGES as i64],
            )
            .map_err(|error| sqlite_error("unable to trim a chat", error))?;
        transaction
            .commit()
            .map_err(|error| sqlite_error("unable to commit a chat write", error))
    }

    fn clear_chat(&self, scope: &ChatScope) -> Result<bool, StorageError> {
        delete_chat(&self.connection, &scope.key())
    }
}

/// Deletes a chat's messages. Outputs: Whether there were any. Side effects: Deletes rows.
pub(super) fn delete_chat(connection: &Connection, chat_key: &str) -> Result<bool, StorageError> {
    connection
        .execute(
            "DELETE FROM chat_messages WHERE chat_key = ?1",
            params![chat_key],
        )
        .map(|removed| removed > 0)
        .map_err(|error| sqlite_error("unable to delete a chat", error))
}

/// Writes an instant as RFC 3339 UTC text with milliseconds.
fn time_text(instant: DateTime<Utc>) -> String {
    instant.to_rfc3339_opts(SecondsFormat::Millis, true)
}
