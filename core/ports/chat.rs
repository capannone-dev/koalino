//! Declares how chats with notes (#97) are kept: per device, sealed, never synced.
//!
//! Messages quote meeting content: implementations seal them at rest and never log them.

use crate::domain::{
    chat::{ChatMessage, ChatScope},
    errors::StorageError,
};

/// Keeps each chat's messages.
pub trait ChatHistoryRepository {
    /// Lists a chat's messages, oldest first.
    ///
    /// Outputs: The messages (none for a new chat), or a storage error. Side effects: Reads and
    /// unseals them.
    fn chat_history(&self, scope: &ChatScope) -> Result<Vec<ChatMessage>, StorageError>;

    /// Adds messages to a chat, keeping at most `MAX_CHAT_MESSAGES` (the oldest go).
    ///
    /// Side effects: Seals and writes them in one transaction.
    fn append_chat_messages(
        &self,
        scope: &ChatScope,
        messages: &[ChatMessage],
    ) -> Result<(), StorageError>;

    /// Deletes a chat's messages. Outputs: Whether there were any.
    fn clear_chat(&self, scope: &ChatScope) -> Result<bool, StorageError>;
}
