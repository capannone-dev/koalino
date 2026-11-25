// JNI bindings to the Rust core (`core/bin/ffi_android.rs`) for calls that must not pass
// through Dart: handing over the database key, the recording notification's actions, which
// run while the app may be in the background (Issue #47), the charging state (#87), and the
// background job that runs a waiting after-meeting pass without Flutter (`WaitingPassJob`, #87).
package io.github.capannone_dev.koalino

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

    /**
     * Configures the core without Dart, for the background job (#87).
     * Inputs: The UTF-8 configuration JSON the app stored. Outputs: 0, 1 for invalid JSON, or 2
     * when this process's app already configured it.
     */
    @JvmStatic
    external fun configure(configurationJson: ByteArray): Int

    /**
     * Opens the database, key store and P2P. Outputs: 0 when open, 6 without a usable key.
     * Side effects: Blocks while they open; never call it on the main thread.
     */
    @JvmStatic
    external fun startRuntime(): Int

    /**
     * Counts the meetings whose speech still awaits the pass. Outputs: the count, or minus the
     * status opening the core failed with. Side effects: May open the core; not on the main thread.
     */
    @JvmStatic
    external fun speechAwaitingPass(): Int

    /**
     * Runs one round of the waiting pass and waits for it, at most `timeoutMs`.
     * Outputs: 0 when the round finished or nothing waits, 2 when the wait ran out, else the
     * core's status. Side effects: Loads the model and re-transcribes; never on the main thread.
     */
    @JvmStatic
    external fun runWaitingPass(timeoutMs: Int): Int

    /**
     * Writes an encrypted backup of all data (#167).
     *
     * Inputs: The data folder and database file name (UTF-8); a descriptor detached from the
     * picked document (the core closes it); the passphrase (UTF-8); the 32-byte database key; a
     * 2-element array receiving files and bytes written. The caller wipes the key and passphrase.
     * Outputs: 0, 1 for a missing argument, or a backup code (20–27, see `backup_commands.rs`).
     * Side effects: Reads the data folder; runs for seconds to minutes, never on the main thread.
     */
    @JvmStatic
    external fun createBackup(
        dataDirectory: ByteArray,
        databaseName: ByteArray,
        outputFd: Int,
        passphrase: ByteArray,
        key: ByteArray,
        summary: LongArray,
    ): Int

    /**
     * Restores a backup into an empty staging folder and gives back its database key (#167).
     *
     * Inputs: A descriptor detached from the picked backup (the core closes it); the passphrase;
     * the staging folder (UTF-8); a 32-byte array receiving the key (the caller wraps, then wipes
     * it); a 2-element array receiving files and bytes restored.
     * Outputs: 0, 1 for a missing argument, or a backup code (20–27).
     * Side effects: Writes the staging folder (removed again on failure); never on the main thread.
     */
    @JvmStatic
    external fun stageRestore(
        inputFd: Int,
        passphrase: ByteArray,
        stagingDirectory: ByteArray,
        keyOut: ByteArray,
        summary: LongArray,
    ): Int
}
