//! JNI entry points for the Android app's Kotlin code.
//!
//! `EncryptionKeyVault.kt` unwraps the database key with the Android Keystore and passes it here
//! directly, so the key never enters the Dart heap (the Dart platform-channel buffer is read-only
//! and cannot be wiped). The bytes are copied into a zeroizing buffer and kept by
//! `ffi::store_host_encryption_key`, the same store as the C entry point
//! `note_taker_provide_encryption_key`.
//!
//! The recording notification (`RecordingService.kt`, Issue #47) pauses, resumes, and stops the
//! recording through the same lifecycle functions Dart calls, so its actions work while the app
//! is in the background or the screen is locked.
//!
//! `MainActivity.kt` reports whether the phone is charging, because a phone runs the
//! after-meeting pass only then (#87). `WaitingPassJob.kt` runs that pass from a JobScheduler
//! job while the phone charges, with the app closed and the screen locked (#87).
//!
//! `BackupChannel.kt` backs up and restores all data from Settings (#167): it hands over the
//! file Android's document picker opened, the passphrase and, for a backup, the unwrapped key;
//! on restore it receives the backup's key in an array it wipes after wrapping it.
//!
//! The Kotlin side is `com.example.note_taker_ui.NativeCore`. It loads this library with
//! `System.loadLibrary("note_taker_core")`, which returns the instance Dart already opened, so
//! both share the runtime's statics.

use std::{fs::File, os::fd::FromRawFd, path::PathBuf};

use jni::{
    Env, EnvUnowned,
    errors::ThrowRuntimeExAndDefault,
    objects::{JByteArray, JClass, JLongArray},
    sys::{jboolean, jint},
};
use zeroize::Zeroizing;

use crate::backup_commands::{create_backup, restore_backup};

use crate::ffi::{
    FFI_INVALID_ARGUMENT, HOST_ENCRYPTION_KEY_LENGTH, note_taker_configure,
    note_taker_pause_recording, note_taker_run_waiting_pass, note_taker_set_charging,
    note_taker_speech_awaiting_pass, note_taker_start_recording, note_taker_start_runtime,
    note_taker_stop_recording, store_host_encryption_key,
};

/// Implements `NativeCore.provideEncryptionKey(key: ByteArray): Int`.
///
/// Inputs: The JNI environment and the 32-byte key array; Kotlin zero-fills it afterwards.
/// Outputs: `0` when stored, `1` for a null array or another length, `3` if the key slot is
/// poisoned. A JNI failure throws a Java `RuntimeException` (no key bytes in its message).
/// Side effects: Replaces the host key for the next runtime start; the temporary copy is wiped.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_provideEncryptionKey<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    key: JByteArray<'caller>,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, jni::errors::Error> {
            if key.is_null() || key.len(env)? != HOST_ENCRYPTION_KEY_LENGTH {
                return Ok(FFI_INVALID_ARGUMENT);
            }
            let mut java_bytes = Zeroizing::new([0_i8; HOST_ENCRYPTION_KEY_LENGTH]);
            key.get_region(env, 0, java_bytes.as_mut_slice())?;
            let key_bytes: Zeroizing<[u8; HOST_ENCRYPTION_KEY_LENGTH]> =
                Zeroizing::new(java_bytes.map(|byte| byte.to_ne_bytes()[0]));
            Ok(store_host_encryption_key(key_bytes.as_slice()))
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Implements `NativeCore.resumeRecording(): Int` for the recording notification (Issue #47).
///
/// Inputs: None (the JNI environment is unused).
/// Outputs: The status of `note_taker_start_recording`, which resumes a paused recording; `7`
/// when the microphone cannot be reopened.
/// Side effects: As `note_taker_start_recording`: turns the microphone on.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_resumeRecording<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> jint {
    note_taker_start_recording()
}

/// Implements `NativeCore.pauseRecording(): Int` for the recording notification.
///
/// Outputs: The status of `note_taker_pause_recording`.
/// Side effects: Pauses the recording and turns the microphone off.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_pauseRecording<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> jint {
    note_taker_pause_recording()
}

/// Implements `NativeCore.stopRecording(): Int` for the recording notification.
///
/// Outputs: The status of `note_taker_stop_recording`.
/// Side effects: Blocks until the remaining audio is transcribed and saved; call it off the
/// main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_stopRecording<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> jint {
    note_taker_stop_recording()
}

/// Implements `NativeCore.setCharging(charging: Boolean): Int` (#87).
///
/// Inputs: Whether the phone is charging.
/// Outputs: The status of `note_taker_set_charging`.
/// Side effects: As `note_taker_set_charging`: may load the model to start a waiting pass, so
/// call it off the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_setCharging<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    charging: jboolean,
) -> jint {
    note_taker_set_charging(jint::from(charging))
}

/// Implements `NativeCore.speechAwaitingPass(): Int`.
///
/// Outputs: As `note_taker_speech_awaiting_pass`: the count, or minus a status.
/// Side effects: Opens the runtime when needed; never call it on the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_speechAwaitingPass<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> jint {
    note_taker_speech_awaiting_pass()
}

/// Implements `NativeCore.runWaitingPass(timeoutMs: Int): Int`.
///
/// Inputs: The longest wait in milliseconds.
/// Outputs: As `note_taker_run_waiting_pass`.
/// Side effects: Loads the model and blocks for a whole round; never call it on the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_runWaitingPass<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    timeout_ms: jint,
) -> jint {
    note_taker_run_waiting_pass(timeout_ms)
}

/// Implements `NativeCore.configure(configurationJson: ByteArray): Int` for the background job,
/// which runs without Dart (#87).
///
/// Inputs: The UTF-8 JSON the app last passed to `note_taker_configure` (no secrets: paths, the
/// device name and peers).
/// Outputs: As `note_taker_configure`: `0`, `1` for invalid JSON or a null array, `2` when the
/// core was already configured (by the app in this process, which is fine).
/// Side effects: Stores the settings for the process. A JNI failure throws a RuntimeException.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_configure<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    configuration_json: JByteArray<'caller>,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, jni::errors::Error> {
            if configuration_json.is_null() {
                return Ok(FFI_INVALID_ARGUMENT);
            }
            let length = configuration_json.len(env)?;
            let mut java_bytes = vec![0_i8; length];
            configuration_json.get_region(env, 0, &mut java_bytes)?;
            let bytes: Vec<u8> = java_bytes
                .into_iter()
                .map(|byte| byte.to_ne_bytes()[0])
                .collect();
            let Ok(configuration) = std::ffi::CString::new(bytes) else {
                return Ok(FFI_INVALID_ARGUMENT);
            };
            // SAFETY: `configuration` is a valid NUL-terminated string that outlives the call.
            Ok(unsafe { note_taker_configure(configuration.as_ptr()) })
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Implements `NativeCore.startRuntime(): Int` for the background job (#87).
///
/// Outputs: As `note_taker_start_runtime`: `0` once open, `6` when the key is missing.
/// Side effects: Opens the database and key store and starts P2P; never call it on the main
/// thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_startRuntime<'caller>(
    _unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
) -> jint {
    note_taker_start_runtime()
}

/// Copies a Java byte array into a wiped buffer; `None` for a null array.
fn java_bytes(
    env: &mut Env<'_>,
    array: &JByteArray<'_>,
) -> Result<Option<Zeroizing<Vec<u8>>>, jni::errors::Error> {
    if array.is_null() {
        return Ok(None);
    }
    let mut java = Zeroizing::new(vec![0_i8; array.len(env)?]);
    array.get_region(env, 0, java.as_mut_slice())?;
    Ok(Some(Zeroizing::new(
        java.iter().map(|byte| byte.to_ne_bytes()[0]).collect(),
    )))
}

/// Decodes a Java byte array as UTF-8 into a wiped string; `None` for null or invalid text.
fn java_text(
    env: &mut Env<'_>,
    array: &JByteArray<'_>,
) -> Result<Option<Zeroizing<String>>, jni::errors::Error> {
    Ok(java_bytes(env, array)?
        .and_then(|bytes| String::from_utf8(bytes.to_vec()).ok().map(Zeroizing::new)))
}

/// Takes over a file descriptor Kotlin detached from a `ParcelFileDescriptor`.
///
/// Safety: Kotlin must pass a descriptor it no longer owns (`detachFd()`); the `File` closes it.
fn owned_file(descriptor: jint) -> Option<File> {
    // SAFETY: Kotlin detached the descriptor and never uses or closes it again.
    (descriptor >= 0).then(|| unsafe { File::from_raw_fd(descriptor) })
}

/// Implements `NativeCore.createBackup(dataDirectory: ByteArray, databaseName: ByteArray,
/// outputFd: Int, passphrase: ByteArray, key: ByteArray, summary: LongArray): Int` (#167).
///
/// Inputs: The data folder and database file name (UTF-8); a writable descriptor detached from
/// the document the owner picked (taken over and closed); the passphrase (UTF-8); the 32-byte
/// database key (Kotlin wipes its copy); a 2-element array receiving files and bytes written.
/// Outputs: `0` on success, `1` for a missing argument, or a `BACKUP_*` code. A JNI failure
/// throws a RuntimeException (no key or passphrase in its message).
/// Side effects: As `backup_commands::create_backup`; runs for seconds to minutes, so call it
/// off the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_createBackup<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    data_directory: JByteArray<'caller>,
    database_name: JByteArray<'caller>,
    output_descriptor: jint,
    passphrase: JByteArray<'caller>,
    key: JByteArray<'caller>,
    summary: JLongArray<'caller>,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, jni::errors::Error> {
            let output = owned_file(output_descriptor);
            let (
                Some(data_directory),
                Some(database_name),
                Some(output),
                Some(passphrase),
                Some(key),
            ) = (
                java_text(env, &data_directory)?,
                java_text(env, &database_name)?,
                output,
                java_text(env, &passphrase)?,
                java_bytes(env, &key)?,
            )
            else {
                return Ok(FFI_INVALID_ARGUMENT);
            };
            let (status, written) = create_backup(
                &PathBuf::from(data_directory.as_str()),
                &database_name,
                output,
                &passphrase,
                &key,
            );
            if let Some(written) = written
                && !summary.is_null()
            {
                summary.set_region(env, 0, &[written.files as i64, written.bytes as i64])?;
            }
            Ok(status)
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}

/// Implements `NativeCore.stageRestore(inputFd: Int, passphrase: ByteArray,
/// stagingDirectory: ByteArray, keyOut: ByteArray, summary: LongArray): Int` (#167).
///
/// Inputs: A readable descriptor detached from the backup the owner picked (taken over and
/// closed); the passphrase (UTF-8); the staging folder (UTF-8, absent or empty); a 32-byte array
/// receiving the backup's database key (Kotlin wraps it with the Keystore, then wipes it); a
/// 2-element array receiving files and bytes restored.
/// Outputs: `0` on success, `1` for a missing argument or a key array of another length, or a
/// `BACKUP_*` code. A JNI failure throws a RuntimeException.
/// Side effects: As `backup_commands::restore_backup`; call it off the main thread.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_example_note_1taker_1ui_NativeCore_stageRestore<'caller>(
    mut unowned_env: EnvUnowned<'caller>,
    _class: JClass<'caller>,
    input_descriptor: jint,
    passphrase: JByteArray<'caller>,
    staging_directory: JByteArray<'caller>,
    key_out: JByteArray<'caller>,
    summary: JLongArray<'caller>,
) -> jint {
    unowned_env
        .with_env(|env| -> Result<jint, jni::errors::Error> {
            let input = owned_file(input_descriptor);
            if key_out.is_null() || key_out.len(env)? != HOST_ENCRYPTION_KEY_LENGTH {
                return Ok(FFI_INVALID_ARGUMENT);
            }
            let (Some(input), Some(passphrase), Some(staging_directory)) = (
                input,
                java_text(env, &passphrase)?,
                java_text(env, &staging_directory)?,
            ) else {
                return Ok(FFI_INVALID_ARGUMENT);
            };
            match restore_backup(
                input,
                &passphrase,
                &PathBuf::from(staging_directory.as_str()),
            ) {
                Ok(restored) => {
                    let java_key: Zeroizing<Vec<i8>> = Zeroizing::new(
                        restored
                            .key
                            .as_bytes()
                            .iter()
                            .map(|byte| i8::from_ne_bytes([*byte]))
                            .collect(),
                    );
                    key_out.set_region(env, 0, &java_key)?;
                    if !summary.is_null() {
                        summary.set_region(
                            env,
                            0,
                            &[restored.files as i64, restored.bytes as i64],
                        )?;
                    }
                    Ok(0)
                }
                Err(status) => Ok(status),
            }
        })
        .resolve::<ThrowRuntimeExAndDefault>()
}
