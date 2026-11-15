//! Keeps the name the owner gave this device (Issue #121) in `<db>.device-name`, plain UTF-8.
//!
//! Without a saved name the device uses `NOTE_TAKER_DEVICE_NAME` (the app passes the host name,
//! or "Android device" on a phone) or the host name. A saved name wins over both, so the owner
//! can tell "Ettore's phone" from "Android device" in every nearby list. The file holds no secret:
//! the name is shown to nearby devices anyway.

use std::path::{Path, PathBuf};

use crate::domain::{errors::DeviceNameError, peer_trust::DeviceName};

/// Why the name could not be saved.
#[derive(Debug, thiserror::Error)]
pub enum DeviceNameSettingsError {
    /// The name is blank, too long, or holds a control character.
    #[error(transparent)]
    Invalid(#[from] DeviceNameError),
    /// The file could not be written.
    #[error("the device name could not be saved: {0}")]
    Storage(#[from] std::io::Error),
}

/// Returns `<db>.device-name`.
pub fn device_name_file_for(database_path: &Path) -> PathBuf {
    let mut name_file = database_path.as_os_str().to_owned();
    name_file.push(".device-name");
    PathBuf::from(name_file)
}

/// Reads the saved name.
///
/// Outputs: The name, or `None` when none is saved or the file holds no valid name.
/// Side effects: Reads the file.
pub fn load_saved_device_name(database_path: &Path) -> Option<DeviceName> {
    std::fs::read_to_string(device_name_file_for(database_path))
        .ok()
        .and_then(|text| DeviceName::parse(&text).ok())
}

/// Validates and saves a name.
///
/// Inputs: The database path and the name as typed.
/// Outputs: The trimmed name, or why it was refused; nothing is written on refusal.
/// Side effects: Replaces the file through a temporary file, so a crash leaves the old name.
pub fn save_device_name(
    database_path: &Path,
    raw_name: &str,
) -> Result<DeviceName, DeviceNameSettingsError> {
    let device_name = DeviceName::parse(raw_name)?;
    let name_file = device_name_file_for(database_path);
    let mut temporary = name_file.as_os_str().to_owned();
    temporary.push(".tmp");
    let temporary = PathBuf::from(temporary);
    std::fs::write(&temporary, device_name.as_str().as_bytes())?;
    std::fs::rename(&temporary, &name_file)?;
    Ok(device_name)
}
