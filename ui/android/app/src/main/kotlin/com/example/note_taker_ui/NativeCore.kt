// JNI bindings to the Rust core (`core/bin/ffi_android.rs`) for calls that must not pass
// through Dart: handing over the database key, the recording notification's actions, which
// run while the app may be in the background (Issue #47), and the charging state (#87).
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

    /** Resumes a paused recording. Outputs: the core's status (0, or 7 without a microphone). */
    @JvmStatic
    external fun resumeRecording(): Int

    /** Pauses the recording and turns the microphone off. Outputs: the core's status. */
    @JvmStatic
    external fun pauseRecording(): Int

    /**
     * Stops the recording. Outputs: the core's status.
     * Side effects: Blocks until the remaining audio is transcribed; never call it on the main
     * thread.
     */
    @JvmStatic
    external fun stopRecording(): Int

    /**
     * Reports whether the phone is charging; the after-meeting pass runs only then (#87).
     * Outputs: the core's status (0).
     * Side effects: May load the Whisper model to start a waiting pass; never call it on the main
     * thread.
     */
    @JvmStatic
    external fun setCharging(charging: Boolean): Int
}
