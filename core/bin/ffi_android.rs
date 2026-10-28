//! JNI entry points for the Android app's Kotlin code.
//!
//! `EncryptionKeyVault.kt` unwraps the database key with the Android Keystore and passes it here
//! directly, so the key never enters the Dart heap (the Dart platform-channel buffer is read-only
//! and cannot be wiped). The bytes are copied into a zeroizing buffer and kept by
//! `ffi::store_host_encryption_key`, the same store as the C entry point
//! `note_taker_provide_encryption_key`.
//!
//! The Kotlin side is `com.example.note_taker_ui.NativeCore`. It loads this library with
//! `System.loadLibrary("note_taker_core")`, which returns the instance Dart already opened, so
//! both share the runtime's statics.

use jni::{
    EnvUnowned,
    errors::ThrowRuntimeExAndDefault,
    objects::{JByteArray, JClass},
    sys::jint,
};
use zeroize::Zeroizing;

use crate::ffi::{FFI_INVALID_ARGUMENT, HOST_ENCRYPTION_KEY_LENGTH, store_host_encryption_key};

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
