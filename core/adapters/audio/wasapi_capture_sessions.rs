//! Lists the apps capturing audio on Windows from the audio session manager (#111).
//!
//! Windows 11 26H2 stopped writing the microphone consent store's `LastUsedTimeStart` and
//! `LastUsedTimeStop` values, so the registry no longer shows a call in progress. WASAPI does: every
//! capture stream belongs to an audio session on its endpoint, and an open, running stream's session
//! is `AudioSessionStateActive`. Each look initializes COM on the calling thread, walks the active
//! capture endpoints' sessions, skips the system-sounds session and inactive ones, and names each
//! owning process by its executable (`msedge.exe`, `ms-teams.exe`). The process ID comes along, so
//! the call watch can leave out the app's own process. Nothing is opened for recording, and nothing
//! found is logged.

use windows::{
    Win32::{
        Foundation::{CloseHandle, S_OK},
        Media::Audio::{
            AudioSessionStateActive, DEVICE_STATE_ACTIVE, IAudioSessionControl2,
            IAudioSessionManager2, IMMDeviceEnumerator, MMDeviceEnumerator, eCapture,
        },
        System::{
            Com::{CLSCTX_ALL, COINIT_MULTITHREADED, CoCreateInstance, CoInitializeEx},
            Threading::{
                OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
                QueryFullProcessImageNameW,
            },
        },
    },
    core::{Interface as _, PWSTR},
};

use super::microphone_users::{MicrophoneUsersError, merge_users, process_name_from_image_path};
use crate::domain::call_watch::MicrophoneUser;

/// The longest image path read, in UTF-16 units (the extended-path limit).
const MAX_IMAGE_PATH: usize = 32_768;

/// Lists the processes with an active capture session on any active capture endpoint.
///
/// Outputs: One user per process name (first session wins), with its process ID; `Tool` when the
/// device enumerator cannot be created or the endpoints cannot be listed. An endpoint or session
/// that cannot be read is skipped. Side effects: Initializes COM on this thread (multithreaded)
/// and queries the audio service.
pub fn active_capture_sessions() -> Result<Vec<MicrophoneUser>, MicrophoneUsersError> {
    // SAFETY: COM initialization has no preconditions; a thread already initialized in another
    // mode returns an error that is ignored, as the audio interfaces work in either mode.
    let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
    // SAFETY: Creates the documented multimedia device enumerator.
    let enumerator: IMMDeviceEnumerator =
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }.map_err(tool)?;
    // SAFETY: Plain enumerator call with documented constants.
    let endpoints =
        unsafe { enumerator.EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE) }.map_err(tool)?;
    // SAFETY: Reads the count of the collection returned above.
    let endpoint_count = unsafe { endpoints.GetCount() }.map_err(tool)?;
    let mut users = Vec::new();
    for endpoint_index in 0..endpoint_count {
        // SAFETY: The index is below the collection's count.
        let Ok(endpoint) = (unsafe { endpoints.Item(endpoint_index) }) else {
            continue;
        };
        // SAFETY: Activates the documented session manager interface on an active endpoint.
        let Ok(manager) = (unsafe { endpoint.Activate::<IAudioSessionManager2>(CLSCTX_ALL, None) })
        else {
            continue;
        };
        users = merge_users(users, endpoint_sessions(&manager));
    }
    Ok(users)
}

/// Lists one endpoint's active, non-system sessions' processes.
fn endpoint_sessions(manager: &IAudioSessionManager2) -> Vec<MicrophoneUser> {
    // SAFETY: Plain session manager calls on an activated interface.
    let Ok(sessions) = (unsafe { manager.GetSessionEnumerator() }) else {
        return Vec::new();
    };
    // SAFETY: Reads the count of the enumerator returned above.
    let session_count = unsafe { sessions.GetCount() }.unwrap_or(0);
    let mut users = Vec::new();
    for session_index in 0..session_count {
        // SAFETY: The index is below the enumerator's count.
        let Ok(session) = (unsafe { sessions.GetSession(session_index) }) else {
            continue;
        };
        // SAFETY: Reads the session's state.
        if unsafe { session.GetState() } != Ok(AudioSessionStateActive) {
            continue;
        }
        let Ok(session) = session.cast::<IAudioSessionControl2>() else {
            continue;
        };
        // SAFETY: Plain session queries; `S_OK` marks the system-sounds session.
        let (system_sounds, process_id) = unsafe {
            (
                session.IsSystemSoundsSession() == S_OK,
                session.GetProcessId(),
            )
        };
        let Ok(process_id) = process_id else {
            continue;
        };
        if system_sounds || process_id == 0 {
            continue;
        }
        if let Some(process) = process_image_name(process_id) {
            users.push(MicrophoneUser {
                process,
                name: String::new(),
                pid: Some(process_id),
            });
        }
    }
    users
}

/// Names a process by its executable's file name.
///
/// Outputs: The file name (`msedge.exe`), or `None` when the process cannot be opened for a
/// limited query (it exited, or it is protected). Side effects: Opens and closes a process handle.
fn process_image_name(process_id: u32) -> Option<String> {
    // SAFETY: Opens a handle with the least access needed; it is closed below on every path.
    let handle =
        unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, process_id) }.ok()?;
    let mut buffer = vec![0u16; MAX_IMAGE_PATH];
    let mut length = MAX_IMAGE_PATH as u32;
    // SAFETY: The buffer holds `length` UTF-16 units; the call writes at most that many and
    // updates `length` to the characters written.
    let read = unsafe {
        QueryFullProcessImageNameW(
            handle,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut length,
        )
    };
    // SAFETY: Closes the handle opened above exactly once.
    let _ = unsafe { CloseHandle(handle) };
    read.ok()?;
    let path = String::from_utf16_lossy(buffer.get(..length as usize)?);
    let name = process_name_from_image_path(&path);
    (!name.is_empty()).then(|| name.to_owned())
}

/// Wraps a COM failure for the caller.
fn tool(error: windows::core::Error) -> MicrophoneUsersError {
    MicrophoneUsersError::Tool(format!("audio session manager: {}", error.code()))
}
