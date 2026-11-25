// Backs up all of the app's data to one encrypted file and restores it (#167), for Settings.
//
// The database key never reaches Dart: for a backup this class unwraps it with the Keystore and
// hands it to the Rust core over JNI; on restore the core hands the backup's key back here, and
// it is wrapped with this app's Keystore key straight into the restored folder. Files are picked
// with Android's document picker, so a backup can go to Downloads, a USB drive or a cloud folder.
package io.github.capannone_dev.koalino

import android.app.Activity
import android.content.ActivityNotFoundException
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.provider.DocumentsContract
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.util.concurrent.Executors

/**
 * Answers `note_taker/backup`:
 *
 * - `pickDestination(suggestedName)`: the document picker to create the backup file; answers its
 *   URI, or null when the owner cancels.
 * - `pickBackup()`: the document picker to choose a backup; answers its URI or null.
 * - `create(uri, passphrase, dataDirectory, databaseName, keyDirectory)`: writes the backup;
 *   answers `{status, files, bytes}` (status 0 on success, else the core's code).
 * - `stage(uri, passphrase, stagingDirectory)`: restores into the staging folder and wraps the
 *   backup's key there; answers `{status, files, bytes}`. Dart swaps the folder in on the next
 *   start (`restore_on_start.dart`).
 * - `restart()`: closes and reopens the app, so a staged restore takes effect.
 * - `pickFolder()`: the folder picker for automatic backups (#170), which also lists cloud
 *   providers' folders; keeps read and write access and answers the tree URI, or null.
 * - `enableAutomatic(uri, passphrase, dataDirectory, keyDirectory)`: turns automatic backups on
 *   and runs the first; answers `{status, files, bytes}`.
 * - `disableAutomatic()`, `automaticStatus()` (`AutomaticBackup.status`), and `backUpNow()`.
 */
class BackupChannel(private val activity: Activity) : MethodChannel.MethodCallHandler {
    /** Runs the core's backup and restore, which take seconds to minutes. */
    private val worker = Executors.newSingleThreadExecutor()
    private val mainThread = Handler(Looper.getMainLooper())
    /** The picker request waiting for its answer, if any. */
    private var pendingPick: MethodChannel.Result? = null
    /** Whether the open picker chooses a folder, whose access must be kept. */
    private var pickingFolder = false

    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "pickDestination" -> pick(result) {
                Intent(Intent.ACTION_CREATE_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = MIME_TYPE
                    putExtra(
                        Intent.EXTRA_TITLE,
                        call.argument<String>("suggestedName") ?: "koalino.koalino-backup",
                    )
                }
            }
            "pickBackup" -> pick(result) {
                Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    // Cloud providers often label the file differently, so accept any type.
                    type = "*/*"
                }
            }
            "create" -> create(call, result)
            "stage" -> stage(call, result)
            "restart" -> restart(result)
            "pickFolder" -> {
                pick(result, folder = true) {
                    Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
                        AutomaticBackup.FOLDER_ACCESS or
                            Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
                    )
                }
            }
            "enableAutomatic" -> enableAutomatic(call, result)
            "disableAutomatic" -> background(result) {
                AutomaticBackup(activity).disable()
                null
            }
            "automaticStatus" -> background(result) { AutomaticBackup(activity).status() }
            "backUpNow" -> background(result) { AutomaticBackup(activity).runNow().toMap() }
            else -> result.notImplemented()
        }
    }

    /** Starts a document picker; the answer comes through `onActivityResult`. */
    private fun pick(result: MethodChannel.Result, folder: Boolean = false, intent: () -> Intent) {
        if (pendingPick != null) {
            result.error("busy", "a file picker is already open", null)
            return
        }
        pendingPick = result
        pickingFolder = folder
        try {
            activity.startActivityForResult(intent(), REQUEST_CODE)
        } catch (error: ActivityNotFoundException) {
            pendingPick = null
            pickingFolder = false
            result.error("no_picker", "this device has no document picker", null)
        }
    }

    /**
     * Receives the picker's answer from `MainActivity`.
     * Outputs: true when the request was this class's. Side effects: answers the waiting call.
     */
    fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?): Boolean {
        if (requestCode != REQUEST_CODE) return false
        val folder = pickingFolder
        pickingFolder = false
        val result = pendingPick ?: return true
        pendingPick = null
        val uri = data?.data
        if (resultCode != Activity.RESULT_OK || uri == null) {
            result.success(null)
            return true
        }
        if (folder) {
            try {
                activity.contentResolver.takePersistableUriPermission(
                    uri,
                    AutomaticBackup.FOLDER_ACCESS,
                )
            } catch (error: SecurityException) {
                result.error("folder_not_kept", "the folder's access could not be kept", null)
                return true
            }
        }
        result.success(uri.toString())
        return true
    }

    private fun create(call: MethodCall, result: MethodChannel.Result) {
        val uri = call.argument<String>("uri")
        val passphrase = call.argument<String>("passphrase")
        val dataDirectory = call.argument<String>("dataDirectory")
        val databaseName = call.argument<String>("databaseName")
        val keyDirectory = call.argument<String>("keyDirectory")
        if (uri == null || passphrase == null || dataDirectory == null || databaseName == null ||
            keyDirectory == null
        ) {
            result.error("invalid_argument", "uri, passphrase and the folders are required", null)
            return
        }
        worker.execute {
            val summary = LongArray(2)
            val passphraseBytes = passphrase.toByteArray(Charsets.UTF_8)
            val answer = try {
                val descriptor = activity.contentResolver.openFileDescriptor(Uri.parse(uri), "wt")
                    ?: error("the backup file could not be opened")
                val key = EncryptionKeyVault(File(keyDirectory)).loadOrCreateKey()
                val status = try {
                    NativeCore.createBackup(
                        dataDirectory.toByteArray(Charsets.UTF_8),
                        databaseName.toByteArray(Charsets.UTF_8),
                        descriptor.detachFd(),
                        passphraseBytes,
                        key,
                        summary,
                    )
                } finally {
                    key.fill(0)
                    descriptor.close()
                }
                if (status != 0) discard(uri)
                Answer.Status(status, summary)
            } catch (error: Exception) {
                discard(uri)
                Answer.Failure(error.javaClass.simpleName)
            } finally {
                passphraseBytes.fill(0)
            }
            mainThread.post { answer.send(result) }
        }
    }

    private fun enableAutomatic(call: MethodCall, result: MethodChannel.Result) {
        val uri = call.argument<String>("uri")
        val passphrase = call.argument<String>("passphrase")
        val dataDirectory = call.argument<String>("dataDirectory")
        val keyDirectory = call.argument<String>("keyDirectory")
        if (uri == null || passphrase == null || dataDirectory == null || keyDirectory == null) {
            result.error("invalid_argument", "uri, passphrase and the folders are required", null)
            return
        }
        background(result) {
            val passphraseBytes = passphrase.toByteArray(Charsets.UTF_8)
            try {
                AutomaticBackup(activity)
                    .enable(Uri.parse(uri), passphraseBytes, dataDirectory, keyDirectory)
                    .toMap()
            } finally {
                passphraseBytes.fill(0)
            }
        }
    }

    /** Runs `work` on the worker thread and answers its value, or `backup_failed`. */
    private fun background(result: MethodChannel.Result, work: () -> Any?) {
        worker.execute {
            val answer: Answer = try {
                Answer.Value(work())
            } catch (error: Exception) {
                Answer.Failure(error.javaClass.simpleName)
            }
            mainThread.post { answer.send(result) }
        }
    }

    /** Deletes the document a failed backup was writing, so no partial file is left behind. */
    private fun discard(uri: String) {
        try {
            DocumentsContract.deleteDocument(activity.contentResolver, Uri.parse(uri))
        } catch (error: Exception) {
            // The provider may not support deleting; the file stays, holding nothing readable.
        }
    }

    private fun stage(call: MethodCall, result: MethodChannel.Result) {
        val uri = call.argument<String>("uri")
        val passphrase = call.argument<String>("passphrase")
        val stagingDirectory = call.argument<String>("stagingDirectory")
        if (uri == null || passphrase == null || stagingDirectory == null) {
            result.error("invalid_argument", "uri, passphrase and stagingDirectory are required", null)
            return
        }
        worker.execute {
            val summary = LongArray(2)
            val key = ByteArray(KEY_LENGTH)
            val passphraseBytes = passphrase.toByteArray(Charsets.UTF_8)
            val staging = File(stagingDirectory)
            val answer = try {
                val descriptor = activity.contentResolver.openFileDescriptor(Uri.parse(uri), "r")
                    ?: error("the backup file could not be opened")
                val status = try {
                    NativeCore.stageRestore(
                        descriptor.detachFd(),
                        passphraseBytes,
                        stagingDirectory.toByteArray(Charsets.UTF_8),
                        key,
                        summary,
                    )
                } finally {
                    descriptor.close()
                }
                if (status == 0) {
                    try {
                        // The restored data opens with the backup's key, kept wrapped by this app.
                        EncryptionKeyVault(File(staging, "keys")).storeRestoredKey(key)
                        keepBackupPassphrase(File(staging, "keys"))
                        File(staging, READY_MARKER).writeText("ready")
                    } catch (error: Exception) {
                        staging.deleteRecursively()
                        throw error
                    }
                }
                Answer.Status(status, summary)
            } catch (error: Exception) {
                Answer.Failure(error.javaClass.simpleName)
            } finally {
                key.fill(0)
                passphraseBytes.fill(0)
            }
            mainThread.post { answer.send(result) }
        }
    }

    /**
     * Copies the automatic backups' wrapped passphrase into the restored key folder, so they keep
     * running after the restore; it is sealed by this app's Keystore key, which stays the same.
     */
    private fun keepBackupPassphrase(restoredKeys: File) {
        val current = AutomaticBackup(activity).keyDirectory ?: return
        val wrapped = File(current, "backup-passphrase.wrapped")
        if (wrapped.exists()) wrapped.copyTo(File(restoredKeys, wrapped.name), overwrite = true)
    }

    /** Reopens the app in a new process, so Dart swaps a staged restore in before opening data. */
    private fun restart(result: MethodChannel.Result) {
        val launch = activity.packageManager.getLaunchIntentForPackage(activity.packageName)
        val component = launch?.component
        if (component == null) {
            result.error("no_launcher", "the app's launcher activity was not found", null)
            return
        }
        result.success(null)
        activity.startActivity(Intent.makeRestartActivityTask(component))
        Runtime.getRuntime().exit(0)
    }

    /** What a background call answers. */
    private sealed class Answer {
        abstract fun send(result: MethodChannel.Result)

        class Status(private val status: Int, private val summary: LongArray) : Answer() {
            override fun send(result: MethodChannel.Result) = result.success(
                mapOf("status" to status, "files" to summary[0], "bytes" to summary[1]),
            )
        }

        class Value(private val value: Any?) : Answer() {
            override fun send(result: MethodChannel.Result) = result.success(value)
        }

        /** A failure outside the core; the message names only the exception's kind. */
        class Failure(private val kind: String) : Answer() {
            override fun send(result: MethodChannel.Result) =
                result.error("backup_failed", kind, null)
        }
    }

    companion object {
        /** The marker `restore_on_start.dart` looks for before swapping the staged folder in. */
        const val READY_MARKER = ".restore-ready"
        private const val MIME_TYPE = "application/octet-stream"
        private const val KEY_LENGTH = 32
        private const val REQUEST_CODE = 0x4b42 // "KB"
    }
}
