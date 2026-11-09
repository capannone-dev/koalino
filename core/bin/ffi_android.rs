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
//! after-meeting pass only then (#87).
//!
//! The Kotlin side is `com.example.note_taker_ui.NativeCore`. It loads this library with
//! `System.loadLibrary("note_taker_core")`, which returns the instance Dart already opened, so
//! both share the runtime's statics.

use jni::{
    EnvUnowned,
    errors::ThrowRuntimeExAndDefault,
    objects::{JByteArray, JClass},
    sys::{jboolean, jint},
};
use zeroize::Zeroizing;

use crate::ffi::{
    FFI_INVALID_ARGUMENT, HOST_ENCRYPTION_KEY_LENGTH, note_taker_pause_recording,
    note_taker_set_charging, note_taker_start_recording, note_taker_stop_recording,
    store_host_encryption_key,
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
