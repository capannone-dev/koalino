//! Resolves the acquisition device ID that every note, log line, and attachment is tagged with.
//!
//! Requirement 3.3 tags every note with its acquisition device. The CLI, daemon, and FFI used to
//! fall back to the shared literal `unconfigured-device` (the daemon even hardcoded
//! `headless-server`), so notes from different machines were indistinguishable. Now
//! `NOTE_TAKER_DEVICE_ID` wins when set; otherwise a `device-<uuid>` is generated once and kept in
//! `<database>.device-id` beside the database, so the ID survives restarts and stays tied to the
//! repository it describes.

use std::{
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use uuid::Uuid;

use crate::domain::models::DeviceId;

/// The environment variable that pins the device ID.
pub const DEVICE_ID_VARIABLE: &str = "NOTE_TAKER_DEVICE_ID";

/// Returns the file that holds the generated device ID for `database_path`.
///
/// Inputs: The SQLite database path. Outputs: `<database path>.device-id`. Side effects: None.
pub fn device_id_file_path(database_path: &Path) -> PathBuf {
    let mut file_name = database_path.as_os_str().to_owned();
    file_name.push(".device-id");
    PathBuf::from(file_name)
}

/// Resolves the device ID from `NOTE_TAKER_DEVICE_ID`, else from the persisted ID file.
///
/// Inputs: The database path. Outputs: See `resolve_device_id`.
/// Side effects: Reads one environment variable; may create the ID file.
pub fn device_id_from_environment(database_path: &Path) -> Result<DeviceId> {
    resolve_device_id(std::env::var(DEVICE_ID_VARIABLE).ok(), database_path)
}

/// Resolves the device ID from a configured value or the ID persisted beside the database.
///
/// Inputs: `configured_device_id` is the variable's value (blank counts as unset);
/// `database_path` is the SQLite file whose identity this is (it need not exist yet).
/// Outputs: The trimmed configured ID, else the persisted one, else a newly generated
/// `device-<uuid v4>`. Errors when the ID file is empty or contains control characters (a new ID
/// would split this device's notes across two identities, so it is never silently replaced), or
/// when it cannot be read or created; errors name the file.
/// Side effects: Creates the ID file on first use. Creation uses `create_new`, so two processes
/// starting at once agree on the first ID written.
pub fn resolve_device_id(
    configured_device_id: Option<String>,
    database_path: &Path,
) -> Result<DeviceId> {
    if let Some(configured_device_id) = configured_device_id
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        return Ok(DeviceId::new(configured_device_id));
    }
    let id_file_path = device_id_file_path(database_path);
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&id_file_path)
    {
        Ok(mut id_file) => {
            let generated_device_id = format!("device-{}", Uuid::new_v4());
            id_file
                .write_all(generated_device_id.as_bytes())
                .with_context(|| format!("unable to write {}", id_file_path.display()))?;
            Ok(DeviceId::new(generated_device_id))
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => read_device_id(&id_file_path),
        Err(error) => {
            Err(error).with_context(|| format!("unable to create {}", id_file_path.display()))
        }
    }
}

/// Reads and validates a persisted device ID.
///
/// Inputs: The ID file path. Outputs: The trimmed ID, or an error naming the file when it is
/// unreadable, blank, or contains control characters. Side effects: Reads the file.
fn read_device_id(id_file_path: &Path) -> Result<DeviceId> {
    let stored_device_id = fs::read_to_string(id_file_path)
        .with_context(|| format!("unable to read {}", id_file_path.display()))?;
    let stored_device_id = stored_device_id.trim();
    if stored_device_id.is_empty() || stored_device_id.chars().any(char::is_control) {
        bail!(
            "{} does not contain a usable device ID; restore it or set {DEVICE_ID_VARIABLE}",
            id_file_path.display()
        );
    }
    Ok(DeviceId::new(stored_device_id))
}

#[cfg(test)]
#[path = "device_identity_tests.rs"]
mod tests;
