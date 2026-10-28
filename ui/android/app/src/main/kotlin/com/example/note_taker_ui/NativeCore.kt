// JNI bindings to the Rust core (`core/bin/ffi_android.rs`) for calls that must not pass
// through Dart, such as handing over the database key.
package com.example.note_taker_ui

/** The Rust core's JNI methods; loading it reuses the library instance Dart already opened. */
object NativeCore {
    init {
        System.loadLibrary("note_taker_core")
    }

    /**
     * Gives the core the 32-byte database key for its next runtime start.
     *
     * Inputs: The key; the caller zero-fills it afterwards.
     * Outputs: 0 when stored, 1 for another length, 3 if the core's key slot is unusable.
     * Side effects: The core keeps a copy in a zeroizing buffer.
     */
    @JvmStatic
    external fun provideEncryptionKey(key: ByteArray): Int
}
