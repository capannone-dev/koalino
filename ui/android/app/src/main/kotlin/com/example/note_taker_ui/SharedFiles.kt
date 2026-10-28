// Shares a file the app exported (the logs, Issue #46) through Android's share sheet. Another app
// can only read a file through a content URI, so `SharedFileProvider` serves the files in the
// app's `cache/shared` folder, read-only, to the app the owner picks; nothing else in the app's
// storage is reachable through it. It is our own minimal provider rather than androidx's
// FileProvider, so the app takes on no new library.
package com.example.note_taker_ui

import android.content.ClipData
import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.FileNotFoundException

/** Answers `shareFile(path, mimeType)` on `note_taker/share`. */
class SharedFiles(private val context: Context) : MethodChannel.MethodCallHandler {
    /**
     * Opens the share sheet for one file of the shared folder.
     *
     * Inputs: `path`, a file directly inside `cache/shared`, and its `mimeType`.
     * Outputs: null once the sheet is shown; `invalid_argument` for a missing argument or a file
     * outside the folder, `share_unavailable` when the sheet cannot open.
     * Side effects: Grants the chosen app temporary read access to that one file.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (call.method != "shareFile") {
            result.notImplemented()
            return
        }
        val path = call.argument<String>("path")
        val mimeType = call.argument<String>("mimeType")
        if (path == null || mimeType == null) {
            result.error("invalid_argument", "path and mimeType are required", null)
            return
        }
        val file = File(path)
        val name = SharedFileProvider.servedName(sharedDirectory(context), file)
        if (name == null || !file.isFile) {
            result.error("invalid_argument", "only files in the shared folder can be shared", null)
            return
        }
        val uri = SharedFileProvider.uriFor(context, name)
        val send = Intent(Intent.ACTION_SEND).apply {
            type = mimeType
            putExtra(Intent.EXTRA_STREAM, uri)
            // The grant travels with the clip, so it reaches the app picked in the chooser.
            clipData = ClipData.newRawUri(name, uri)
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
        }
        try {
            context.startActivity(Intent.createChooser(send, null).apply {
                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                if (context !is android.app.Activity) {
                    addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                }
            })
            result.success(null)
        } catch (error: RuntimeException) {
            result.error("share_unavailable", error.message, null)
        }
    }

    companion object {
        /** The only folder the provider serves; Dart's `getTemporaryDirectory()` is `cacheDir`. */
        fun sharedDirectory(context: Context): File = File(context.cacheDir, "shared")
    }
}

/**
 * Serves the files of `cache/shared` read-only, as
 * `content://<applicationId>.shared/<file name>`. Not exported: apps reach a file only through
 * the URI grant of a share.
 */
class SharedFileProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    /** Opens a served file for reading; any other mode or file is refused. */
    override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
        if (mode != "r") {
            throw SecurityException("shared files are read-only")
        }
        return ParcelFileDescriptor.open(servedFile(uri), ParcelFileDescriptor.MODE_READ_ONLY)
    }

    /** Answers the display name and size that receiving apps ask for. */
    override fun query(
        uri: Uri,
        projection: Array<out String>?,
        selection: String?,
        selectionArgs: Array<out String>?,
        sortOrder: String?,
    ): Cursor {
        val file = servedFile(uri)
        val columns = projection ?: arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE)
        val row = columns.map { column ->
            when (column) {
                OpenableColumns.DISPLAY_NAME -> file.name
                OpenableColumns.SIZE -> file.length()
                else -> null
            }
        }
        return MatrixCursor(columns, 1).apply { addRow(row) }
    }

    /** Exported logs are JSON lines, offered as plain text so mail and chat apps accept them. */
    override fun getType(uri: Uri): String = "text/plain"

    override fun insert(uri: Uri, values: ContentValues?): Uri? =
        throw UnsupportedOperationException("shared files are read-only")

    override fun update(
        uri: Uri,
        values: ContentValues?,
        selection: String?,
        selectionArgs: Array<out String>?,
    ): Int = throw UnsupportedOperationException("shared files are read-only")

    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int =
        throw UnsupportedOperationException("shared files are read-only")

    /** Resolves a URI to a file directly inside the shared folder, or throws. */
    private fun servedFile(uri: Uri): File {
        val context = context ?: throw FileNotFoundException("no context")
        val name = uri.pathSegments.singleOrNull() ?: throw FileNotFoundException("not served")
        val directory = SharedFiles.sharedDirectory(context)
        val file = File(directory, name)
        if (servedName(directory, file) != name || !file.isFile) {
            throw FileNotFoundException("not served")
        }
        return file
    }

    companion object {
        /** The content URI of a served file. */
        fun uriFor(context: Context, name: String): Uri =
            Uri.Builder()
                .scheme("content")
                .authority("${context.packageName}.shared")
                .appendPath(name)
                .build()

        /**
         * The name `file` is served under, or null when it is not directly inside `directory`
         * (after resolving `..` and links), so no other file can be reached.
         */
        fun servedName(directory: File, file: File): String? {
            val canonicalFile = file.canonicalFile
            return if (canonicalFile.parentFile == directory.canonicalFile) canonicalFile.name else null
        }
    }
}
