// Serves the `note_taker/whisper_model` channel: `install(targetPath)` copies the Whisper model
// shipped in the APK (`assets/models/ggml-base.bin`) into the app's data directory, where the
// Rust core reads it by path, so a fresh install transcribes offline with no setup. The copy is
// skipped when the target already has the asset's size, and written through a temporary file
// so an interrupted copy never leaves a truncated model behind.
package com.example.note_taker_ui

import android.content.res.AssetManager
import android.os.Handler
import android.os.Looper
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.FileNotFoundException
import java.io.IOException
import java.util.concurrent.Executors

/** Installs the shipped model from `assets`. */
class WhisperModel(private val assets: AssetManager) : MethodChannel.MethodCallHandler {
    /** Copies run off the main thread, one at a time. */
    private val executor = Executors.newSingleThreadExecutor()
    private val mainThread = Handler(Looper.getMainLooper())

    /**
     * Answers `install(targetPath)` with true when a model is at the target afterwards.
     *
     * Side effects: May copy about 150 MB on a background thread. Without a shipped model it
     * answers whether the target already holds one (e.g. placed there by the owner). A failed
     * copy answers `copy_failed`, naming the step only.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (call.method != "install") {
            result.notImplemented()
            return
        }
        val targetPath = call.argument<String>("targetPath")
        if (targetPath == null) {
            result.error("invalid_argument", "targetPath is required", null)
            return
        }
        executor.execute {
            try {
                val installed = install(File(targetPath))
                mainThread.post { result.success(installed) }
            } catch (error: IOException) {
                mainThread.post { result.error("copy_failed", error.message, null) }
            }
        }
    }

    /** Copies the asset unless the target already matches it; returns whether a model exists. */
    private fun install(target: File): Boolean {
        val assetLength = try {
            assets.openFd(ASSET_PATH).use { it.length }
        } catch (_: FileNotFoundException) {
            return target.isFile
        }
        if (target.isFile && target.length() == assetLength) {
            return true
        }
        target.parentFile?.mkdirs()
        val partial = File(target.path + ".partial")
        assets.open(ASSET_PATH).use { input ->
            partial.outputStream().use { output -> input.copyTo(output, BUFFER_SIZE) }
        }
        if (!partial.renameTo(target)) {
            partial.delete()
            throw IOException("unable to move the model into place")
        }
        return true
    }

    private companion object {
        const val ASSET_PATH = "models/ggml-base.bin"
        const val BUFFER_SIZE = 1 shl 20
    }
}
