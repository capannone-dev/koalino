// Receives files, e-mails and text that other apps share to Note Taker (Issue #85). Android hands
// them over as content URIs that are readable only while the share lasts, so each share is copied
// at once into its own folder under `cache/incoming`, and Dart is told on `note_taker/incoming_shares`.
// Dart asks which project they belong to, has Rust copy them into it, then releases the folder.
// Only `content://` URIs are read: a `file://` URI from another app could name this app's own
// private files (its database, keys or logs), which must never end up in a project.
package com.example.note_taker_ui

import android.content.ContentResolver
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.OpenableColumns
import android.webkit.MimeTypeMap
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.IOException
import java.util.concurrent.Executors

/** Copies shares into the cache and answers `takePending` and `release(directory)`. */
class IncomingShares(private val context: Context) : MethodChannel.MethodCallHandler {
    /** Copying runs off the main thread: a shared e-mail or PDF can be large. */
    private val copyExecutor = Executors.newSingleThreadExecutor()

    private val mainThread = Handler(Looper.getMainLooper())

    /** The latest copied share Dart has not taken yet, or null. Main thread only. */
    private var pendingShare: Map<String, Any>? = null

    /** The channel to tell Dart a share arrived while it runs; null until the engine exists. */
    var channel: MethodChannel? = null

    init {
        copyExecutor.execute { removeStaleFolders() }
    }

    /**
     * Copies what `intent` shares, if it is a share.
     *
     * Inputs: The intent that started or reached the activity.
     * Outputs: Whether it was a share (the caller then clears it so it is not imported twice).
     * Side effects: Reads the shared URIs and writes a new folder under `cache/incoming` in the
     * background, then stores it as pending and calls `sharesArrived` on the channel.
     */
    fun receive(intent: Intent?): Boolean {
        val action = intent?.action
        if (action != Intent.ACTION_SEND && action != Intent.ACTION_SEND_MULTIPLE) {
            return false
        }
        val streams = sharedStreams(intent)
        val sharedText = intent.getCharSequenceExtra(Intent.EXTRA_TEXT)?.toString()
        val subject = intent.getStringExtra(Intent.EXTRA_SUBJECT)
        copyExecutor.execute {
            val share = copyShare(streams, sharedText, subject)
            mainThread.post {
                pendingShare = share
                channel?.invokeMethod("sharesArrived", null)
            }
        }
        return true
    }

    /**
     * Answers Dart.
     *
     * `takePending`: the pending share as `{directory, paths, failedCount}`, or null; it is then
     * no longer pending. `release(directory)`: deletes that share's folder; `invalid_argument` for
     * a folder that is not a share's.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "takePending" -> {
                result.success(pendingShare)
                pendingShare = null
            }
            "release" -> {
                val directory = call.argument<String>("directory")?.let(::File)
                if (directory == null || directory.parentFile?.canonicalFile != incomingRoot().canonicalFile) {
                    result.error("invalid_argument", "not a shared folder", null)
                    return
                }
                copyExecutor.execute { directory.deleteRecursively() }
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /** `cache/incoming`, holding one folder per share. */
    private fun incomingRoot(): File = File(context.cacheDir, "incoming")

    /** The URIs the intent shares as streams, in order; empty for a text-only share. */
    private fun sharedStreams(intent: Intent): List<Uri> {
        val uris = if (intent.action == Intent.ACTION_SEND_MULTIPLE) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableArrayListExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableArrayListExtra<Uri>(Intent.EXTRA_STREAM)
            }
        } else {
            val single = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                intent.getParcelableExtra(Intent.EXTRA_STREAM, Uri::class.java)
            } else {
                @Suppress("DEPRECATION")
                intent.getParcelableExtra<Uri>(Intent.EXTRA_STREAM)
            }
            listOfNotNull(single)
        }
        return uris.orEmpty().filter { it.scheme == ContentResolver.SCHEME_CONTENT }
    }

    /**
     * Copies one share into a new folder.
     *
     * Inputs: The content URIs, and the shared text and subject (a text-only share becomes a
     * `.txt` file named after the subject).
     * Outputs: `{directory, paths, failedCount}`; a file that cannot be read or is larger than
     * the API accepts is counted as failed and leaves nothing behind.
     * Side effects: Writes files under `cache/incoming`.
     */
    private fun copyShare(streams: List<Uri>, sharedText: String?, subject: String?): Map<String, Any> {
        val directory = File(incomingRoot(), "share-${System.currentTimeMillis()}-${System.nanoTime()}")
        val paths = mutableListOf<String>()
        var failedCount = 0
        if (!directory.mkdirs()) {
            return mapOf("directory" to directory.path, "paths" to paths, "failedCount" to streams.size)
        }
        val usedNames = mutableSetOf<String>()
        streams.forEachIndexed { index, uri ->
            val target = File(directory, uniqueName(displayName(uri, index), usedNames))
            if (copyLimited(uri, target)) paths.add(target.path) else failedCount++
        }
        if (streams.isEmpty() && !sharedText.isNullOrBlank()) {
            val title = sanitizeName(subject?.takeIf { it.isNotBlank() } ?: "Shared text")
            val target = File(directory, uniqueName("$title.txt", usedNames))
            try {
                target.writeText(sharedText)
                paths.add(target.path)
            } catch (error: IOException) {
                failedCount++
            }
        }
        return mapOf("directory" to directory.path, "paths" to paths, "failedCount" to failedCount)
    }

    /** Copies at most `MAX_SHARED_FILE_BYTES`; returns false, deleting the copy, otherwise. */
    private fun copyLimited(uri: Uri, target: File): Boolean = try {
        val input = context.contentResolver.openInputStream(uri) ?: throw IOException("no stream")
        var copiedBytes = 0L
        input.use { source ->
            target.outputStream().use { sink ->
                val buffer = ByteArray(64 * 1024)
                while (true) {
                    val read = source.read(buffer)
                    if (read < 0) break
                    copiedBytes += read
                    if (copiedBytes > MAX_SHARED_FILE_BYTES) throw IOException("too large")
                    sink.write(buffer, 0, read)
                }
            }
        }
        true
    } catch (error: Exception) {
        // IOException, or SecurityException when the sender's grant does not cover the URI.
        target.delete()
        false
    }

    /** The name the sending app gives the file, with an extension from its type when missing. */
    private fun displayName(uri: Uri, index: Int): String {
        val queried = try {
            context.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)
                ?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
        } catch (error: Exception) {
            null
        }
        val name = sanitizeName(queried ?: "Shared file ${index + 1}")
        if (name.contains('.')) return name
        val mimeType = context.contentResolver.getType(uri)
        // Android's table has no extension for e-mails; Rust recognises them by `.eml`.
        val extension = if (mimeType == "message/rfc822") {
            "eml"
        } else {
            mimeType?.let { MimeTypeMap.getSingleton().getExtensionFromMimeType(it) }
        }
        return if (extension == null) name else "$name.$extension"
    }

    /** Deletes share folders older than a day, left by a share Dart never took. */
    private fun removeStaleFolders() {
        val cutoff = System.currentTimeMillis() - STALE_AFTER_MILLIS
        incomingRoot().listFiles()?.filter { it.lastModified() < cutoff }?.forEach { it.deleteRecursively() }
    }

    companion object {
        /** The largest file Rust stores in a project (`MAX_ATTACHMENT_BYTES`, 100 MiB). */
        const val MAX_SHARED_FILE_BYTES = 100L * 1024 * 1024

        private const val STALE_AFTER_MILLIS = 24L * 60 * 60 * 1000

        /**
         * Keeps a received name to one safe path component: no separators, control characters
         * or leading dots, at most 120 characters; empty names become "Shared file".
         */
        fun sanitizeName(raw: String): String {
            val cleaned = raw.map { if (it == '/' || it == '\\' || it.isISOControl()) '_' else it }
                .joinToString("").trim().trimStart('.').take(120)
            return cleaned.ifEmpty { "Shared file" }
        }

        /** `name`, or `name (2)`, `name (3)`… when an earlier file of the share took it. */
        fun uniqueName(name: String, usedNames: MutableSet<String>): String {
            var candidate = name
            var counter = 2
            while (!usedNames.add(candidate.lowercase())) {
                val dot = name.lastIndexOf('.')
                candidate = if (dot > 0) {
                    "${name.substring(0, dot)} ($counter)${name.substring(dot)}"
                } else {
                    "$name ($counter)"
                }
                counter++
            }
            return candidate
        }
    }
}
