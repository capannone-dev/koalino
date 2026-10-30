//! Keeps the owner's default transcription language for the apps (Issue #62), in a small file
//! beside the database (`<db>.transcription-language`), as the log level is kept. It is read when
//! a recording starts and has no secret in it, so it needs neither the database nor its key.

use std::{
    ffi::{CStr, CString, c_char, c_int},
    path::PathBuf,
};

use crate::{
    domain::transcription_language::TranscriptionLanguage,
    ffi::{FFI_INVALID_ARGUMENT, FFI_OPERATION_REJECTED, FFI_SUCCESS},
    ffi_settings::app_setting,
    pipeline_composition::WHISPER_LANGUAGE_VARIABLE,
};

/// The file holding the choice.
///
/// Inputs: The settings lookup. Outputs: `<db>.transcription-language`. Side effects: None.
fn default_language_file(setting: &dyn Fn(&str) -> Option<String>) -> PathBuf {
    let mut path = setting("NOTE_TAKER_DATABASE_PATH")
        .unwrap_or_else(|| "note_taker.sqlite3".to_owned())
        .into_bytes();
    path.extend_from_slice(b".transcription-language");
    PathBuf::from(String::from_utf8_lossy(&path).into_owned())
}

/// Reads the default language.
///
/// Inputs: The settings lookup.
/// Outputs: The stored choice, else `NOTE_TAKER_WHISPER_LANGUAGE`, else `Auto`. A missing or damaged
/// file, or an invalid variable, counts as not set, so recordings can always start.
/// Side effects: Reads the file.
pub fn default_language(setting: &dyn Fn(&str) -> Option<String>) -> TranscriptionLanguage {
    std::fs::read_to_string(default_language_file(setting))
        .ok()
        .and_then(|text| TranscriptionLanguage::parse(&text).ok())
        .or_else(|| {
            setting(WHISPER_LANGUAGE_VARIABLE)
                .and_then(|text| TranscriptionLanguage::parse(&text).ok())
        })
        .unwrap_or(TranscriptionLanguage::Auto)
}

/// Stores the default language.
///
/// Inputs: The settings lookup and the choice.
/// Outputs: Success or the I/O error.
/// Side effects: Replaces the file through a temporary one, so a crash cannot damage it.
pub fn save_default_language(
    setting: &dyn Fn(&str) -> Option<String>,
    language: TranscriptionLanguage,
) -> std::io::Result<()> {
    let path = default_language_file(setting);
    let mut partial = path.clone().into_os_string();
    partial.push(".partial");
    std::fs::write(&partial, language.as_str())?;
    std::fs::rename(&partial, &path)
}

/// Returns the default language for Settings and the Record screen.
///
/// Inputs: None. Outputs: An owned C string (free with `note_taker_free_string`): `auto` or a
/// Whisper code. Null only if allocation fails. Side effects: Reads the file.
#[unsafe(no_mangle)]
pub extern "C" fn note_taker_get_default_language() -> *mut c_char {
    CString::new(default_language(&app_setting).as_str())
        .map_or(std::ptr::null_mut(), CString::into_raw)
}

/// Stores the default language.
///
/// Inputs: `language` is `auto` or a Whisper code. Outputs: `FFI_SUCCESS`, `FFI_INVALID_ARGUMENT`
/// (null, not UTF-8, or unknown), or `FFI_OPERATION_REJECTED` when it cannot be stored.
/// Side effects: Writes the file; applies from the next recording.
///
/// # Safety
/// `language` must be null or point to a valid NUL-terminated C string for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn note_taker_set_default_language(language: *const c_char) -> c_int {
    if language.is_null() {
        return FFI_INVALID_ARGUMENT;
    }
    let parsed = unsafe { CStr::from_ptr(language) }
        .to_str()
        .ok()
        .and_then(|text| TranscriptionLanguage::parse(text).ok());
    let Some(parsed) = parsed else {
        return FFI_INVALID_ARGUMENT;
    };
    match save_default_language(&app_setting, parsed) {
        Ok(()) => FFI_SUCCESS,
        Err(error) => {
            tracing::warn!(error = %error, "unable to store the default transcription language");
            FFI_OPERATION_REJECTED
        }
    }
}

#[cfg(test)]
#[path = "ffi_language_tests.rs"]
mod tests;
