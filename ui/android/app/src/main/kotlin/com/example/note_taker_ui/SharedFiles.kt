// Shares a file the app exported (the logs, Issue #46) through Android's share sheet. Another app
// can only read a file through a content URI, so `SharedFileProvider` serves the files in the
// app's `cache/shared` folder, read-only, to the app the owner picks; nothing else in the app's
// storage is reachable through it. It is our own minimal provider rather than androidx's
// FileProvider, so the app takes on no new library.
//
// It also hands a meeting to the owner's own AI app (#114): `shareFiles` sends a message and the
// files of `cache/shared/handoff` together, to the Claude or ChatGPT app when one is named and
// installed, else through the share sheet; `installedApps` says which of those apps are there.
package com.example.note_taker_ui

import android.content.ClipData
import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.FileNotFoundException
import java.net.URLConnection

/** Answers `shareFile`, `shareFiles` and `installedApps` on `note_taker/share`. */
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
        when (call.method) {
            "shareFile" -> {}
            "shareFiles" -> return shareFiles(call, result)
            "installedApps" -> return result.success(
                AI_APPS.filter { packageName -> isInstalled(packageName) },
            )
            else -> return result.notImplemented()
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

    /**
     * Sends a message and several files of the hand-off folder (#114).
     *
     * Inputs: `paths` (files directly inside `cache/shared/handoff`), `text`, and optionally
     * `packageName`, one of `AI_APPS`. Outputs: null once the app or sheet opened;
     * `invalid_argument` for a missing argument or a file outside the folder,
     * `share_unavailable` when nothing can take the share.
     * Side effects: Grants the receiving app temporary read access to those files only. A named
     * app that is not installed falls back to the share sheet.
     */
    private fun shareFiles(call: MethodCall, result: MethodChannel.Result) {
        val paths = call.argument<List<String>>("paths")
        val text = call.argument<String>("text")
        if (paths == null || paths.isEmpty() || text == null) {
            result.error("invalid_argument", "paths and text are required", null)
            return
        }
        val folder = handoffDirectory(context)
        val uris = paths.map { path ->
            val file = File(path)
            val name = SharedFileProvider.servedName(folder, file)
            if (name == null || !file.isFile) {
                result.error("invalid_argument", "only hand-off files can be shared", null)
                return
            }
            SharedFileProvider.uriFor(context, name, HANDOFF_FOLDER)
        }
        val send = Intent(if (uris.size == 1) Intent.ACTION_SEND else Intent.ACTION_SEND_MULTIPLE).apply {
            type = "*/*"
            putExtra(Intent.EXTRA_TEXT, text)
            if (uris.size == 1) {
                putExtra(Intent.EXTRA_STREAM, uris[0])
            } else {
                putParcelableArrayListExtra(Intent.EXTRA_STREAM, ArrayList(uris))
            }
            // The grant travels with the clip, so it reaches the receiving app.
            clipData = ClipData.newRawUri("", uris[0]).apply {
                uris.drop(1).forEach { uri -> addItem(ClipData.Item(uri)) }
            }
            addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
            if (context !is android.app.Activity) {
                addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
            }
        }
        val packageName = call.argument<String>("packageName")
            ?.takeIf { name -> name in AI_APPS && isInstalled(name) }
        try {
            if (packageName != null) {
                context.startActivity(send.setPackage(packageName))
            } else {
                context.startActivity(Intent.createChooser(send, null).apply {
                    addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    if (context !is android.app.Activity) {
                        addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                    }
                })
            }
            result.success(null)
        } catch (error: RuntimeException) {
            result.error("share_unavailable", error.message, null)
        }
    }

    /** Whether an app is installed; the manifest's `<queries>` makes these visible. */
    private fun isInstalled(packageName: String): Boolean = try {
        context.packageManager.getPackageInfo(packageName, 0)
        true
    } catch (_: PackageManager.NameNotFoundException) {
        false
    }

    companion object {
        /** The AI apps a meeting can be handed to directly: Claude and ChatGPT (#114). */
        val AI_APPS = listOf("com.anthropic.claude", "com.openai.chatgpt")

        /** The hand-off subfolder of the shared folder, and its URI path segment. */
        const val HANDOFF_FOLDER = "handoff"

        /** The only folder the provider serves; Dart's `getTemporaryDirectory()` is `cacheDir`. */
        fun sharedDirectory(context: Context): File = File(context.cacheDir, "shared")

        /** The hand-off files' folder (#114), which the core empties before each hand-off. */
        fun handoffDirectory(context: Context): File = File(sharedDirectory(context), HANDOFF_FOLDER)
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

    /**
     * Exported logs (JSON lines) and Markdown are offered as plain text, so mail and chat apps
     * accept them; other hand-off files by their extension.
     */
    override fun getType(uri: Uri): String {
        val name = uri.lastPathSegment ?: return "text/plain"
        if (name.endsWith(".md") || name.endsWith(".jsonl") || name.endsWith(".log")) {
            return "text/plain"
        }
        return URLConnection.guessContentTypeFromName(name) ?: "application/octet-stream"
    }

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

    /**
     * Resolves a URI to a file directly inside the shared folder, or inside its hand-off
     * subfolder (`handoff/<name>`), or throws.
     */
    private fun servedFile(uri: Uri): File {
        val context = context ?: throw FileNotFoundException("no context")
        val segments = uri.pathSegments
        val (directory, name) = when {
            segments.size == 1 -> SharedFiles.sharedDirectory(context) to segments[0]
            segments.size == 2 && segments[0] == SharedFiles.HANDOFF_FOLDER ->
                SharedFiles.handoffDirectory(context) to segments[1]
            else -> throw FileNotFoundException("not served")
        }
        val file = File(directory, name)
        if (servedName(directory, file) != name || !file.isFile) {
            throw FileNotFoundException("not served")
        }
        return file
    }

    companion object {
        /** The content URI of a served file, in the shared folder or one of its subfolders. */
        fun uriFor(context: Context, name: String, folder: String? = null): Uri =
            Uri.Builder()
                .scheme("content")
                .authority("${context.packageName}.shared")
                .apply { if (folder != null) appendPath(folder) }
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
