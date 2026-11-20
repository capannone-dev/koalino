//! Declares where a device keeps the owner's prompts that paired devices share (#145).
//!
//! The P2P service reads them to send to each connected device, and merges what the devices send
//! with `SharedPrompts::merge`. The host keeps them with its language model settings.

use crate::domain::{errors::StorageError, shared_prompts::SharedPrompts};

/// Reads and merges this device's shared prompts.
pub trait SharedPromptsSync: Send {
    /// Reads the prompts as they are now.
    ///
    /// Outputs: Both prompts with when they changed, or why they could not be read.
    /// Side effects: Reads the store.
    fn shared_prompts(&self) -> Result<SharedPrompts, StorageError>;

    /// Keeps the newer version of each prompt another device sent.
    ///
    /// Inputs: The device's prompts, already checked. Outputs: Whether anything changed here.
    /// Side effects: Writes the store when a prompt changes.
    fn merge_shared_prompts(&self, incoming: &SharedPrompts) -> Result<bool, StorageError>;
}
