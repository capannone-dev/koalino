//! File-system helpers shared by the attachment stores (screen-share slides, Issue #49; imported
//! files and camera photos, Issue #54).
//!
//! Attachment folders are named after meeting IDs, so `is_plain_folder_name` keeps a hostile ID
//! from reaching outside the attachment root. Folders are created 0700 and files 0600 on Unix,
//! and a file is never overwritten. Errors name the operation, never file contents.

use std::{
    fs::{DirBuilder, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use crate::domain::errors::StorageError;

/// Tells whether a meeting ID can name a folder without reaching outside its parent.
///
/// Inputs: A meeting ID.
/// Outputs: `true` for a non-empty ID of ASCII letters, digits, `-` and `_` only.
/// Side effects: None.
pub fn is_plain_folder_name(meeting_id: &str) -> bool {
    !meeting_id.is_empty()
        && meeting_id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
}

/// Removes a deleted meeting's folder in an attachment store.
///
/// Inputs: The store's folder and the meeting. An ID that cannot name a folder never had one.
/// Outputs: Success (also when there is no folder), or the file error.
/// Side effects: Deletes the folder and its files. Files linked from elsewhere are not in it.
pub fn remove_meeting_folder(attachment_directory: &Path, meeting_id: &str) -> std::io::Result<()> {
    if !is_plain_folder_name(meeting_id) {
        return Ok(());
    }
    match std::fs::remove_dir_all(attachment_directory.join(meeting_id)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// Creates `directory` (and its parents), readable only by the user on Unix.
///
/// Inputs: The folder to create. Outputs: Success (also when it exists) or a rejection.
/// Side effects: Creates folders.
pub fn create_private_directory(directory: &Path) -> Result<(), StorageError> {
    let mut directory_builder = DirBuilder::new();
    directory_builder.recursive(true);
    #[cfg(unix)]
    std::os::unix::fs::DirBuilderExt::mode(&mut directory_builder, 0o700);
    directory_builder
        .create(directory)
        .map_err(|error| file_error("unable to create the attachment directory", &error))
}

/// Writes a new file, readable only by the user on Unix, from `contents`; never overwrites.
///
/// Inputs: The new file's path and a reader of its contents.
/// Outputs: The number of bytes written, or a rejection. A partly written file is removed.
/// Side effects: Creates, writes and syncs one file.
pub fn write_private_file(path: &Path, contents: &mut impl Read) -> Result<u64, StorageError> {
    let mut open_options = OpenOptions::new();
    open_options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut open_options, 0o600);
    let mut file = open_options
        .open(path)
        .map_err(|error| file_error("unable to create the attachment file", &error))?;
    let written = std::io::copy(contents, &mut file).and_then(|byte_count| {
        file.flush()
            .and_then(|()| file.sync_all())
            .map(|()| byte_count)
    });
    match written {
        Ok(byte_count) => Ok(byte_count),
        Err(error) => {
            drop(file);
            let _ = std::fs::remove_file(path);
            Err(file_error("unable to write the attachment file", &error))
        }
    }
}

/// Builds a storage rejection for a file-system failure.
///
/// Inputs: What was being done and the I/O error. Outputs: The rejection. Side effects: None.
pub fn file_error(operation: &str, error: &std::io::Error) -> StorageError {
    StorageError::Rejected {
        reason: format!("{operation}: {error}"),
    }
}
