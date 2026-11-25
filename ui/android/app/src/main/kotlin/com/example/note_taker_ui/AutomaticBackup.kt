// Automatic backups (#170): a daily JobScheduler job writes the same encrypted backup as
// Settings' Back up all data (#167) into a folder the owner chose with Android's folder picker.
//
// The folder can be on the phone, on a USB drive or at a cloud provider whose app offers a
// document provider (Google Drive, Nextcloud, OneDrive, Dropbox...): the provider uploads a file
// that is already encrypted, and no Koalino server takes part. The job runs while the phone
// charges, without Flutter: it unwraps the database key and the backup passphrase with the
// Keystore and calls the core over JNI. The last `KEPT_BACKUPS` automatic backups are kept.
package com.example.note_taker_ui

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.provider.DocumentsContract
import android.util.Log
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/** The settings and the work of automatic backups. */
class AutomaticBackup(private val context: Context) {
    private val preferences =
        context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)

    /** The folder's tree URI, or null when automatic backups are off. */
    val folder: Uri?
        get() = preferences.getString(FOLDER_KEY, null)?.let(Uri::parse)

    /** The key folder whose vault holds the database key and the backup passphrase. */
    val keyDirectory: String?
        get() = preferences.getString(KEY_DIRECTORY_KEY, null)

    /**
     * Turns automatic backups on and runs one now.
     *
     * Inputs: The folder's tree URI (read and write permission already persisted); the UTF-8
     * passphrase, which the caller wipes; the data and key folders.
     * Outputs: The first backup's result. Throws when the passphrase cannot be stored.
     * Side effects: Stores the passphrase wrapped by the Keystore, writes the preferences,
     * schedules the daily job and writes a backup into the folder.
     */
    fun enable(folder: Uri, passphrase: ByteArray, dataDirectory: String, keyDirectory: String):
        Result {
        EncryptionKeyVault(File(keyDirectory)).storeBackupPassphrase(passphrase)
        preferences.edit()
            .putString(FOLDER_KEY, folder.toString())
            .putString(DATA_DIRECTORY_KEY, dataDirectory)
            .putString(KEY_DIRECTORY_KEY, keyDirectory)
            .apply()
        schedule(context)
        return runNow()
    }

    /**
     * Turns automatic backups off; the backups already written stay in the folder.
     *
     * Side effects: Cancels the job, deletes the stored passphrase, gives up the folder
     * permission and clears the preferences.
     */
    fun disable() {
        context.getSystemService(JobScheduler::class.java)?.cancel(JOB_ID)
        keyDirectory?.let { EncryptionKeyVault(File(it)).forgetBackupPassphrase() }
        folder?.let {
            try {
                context.contentResolver.releasePersistableUriPermission(it, FOLDER_ACCESS)
            } catch (error: SecurityException) {
                // The permission was already gone.
            }
        }
        preferences.edit().clear().apply()
    }

    /** What Settings shows: whether it is on, the folder's name, and the last run. */
    fun status(): Map<String, Any?> = mapOf(
        "enabled" to (folder != null),
        "folder" to folder?.let(::folderLabel),
        "lastTime" to preferences.getLong(LAST_TIME_KEY, 0L).takeIf { it > 0 },
        "lastStatus" to preferences.getInt(LAST_STATUS_KEY, 0),
        "lastFiles" to preferences.getLong(LAST_FILES_KEY, 0L),
        "lastBytes" to preferences.getLong(LAST_BYTES_KEY, 0L),
    )

    /**
     * Writes one backup into the folder now and prunes the old ones.
     *
     * Outputs: The core's status and what was written; `NOT_SET_UP` when automatic backups are
     * off or the passphrase is gone, `FOLDER_UNAVAILABLE` when the folder cannot be written.
     * Side effects: Creates `koalino-auto-<time>.koalino-backup` (deleted again on failure),
     * deletes automatic backups beyond `KEPT_BACKUPS`, records the result. Never call it on the
     * main thread. Logs statuses and counts only.
     */
    fun runNow(): Result {
        val result = writeBackup()
        preferences.edit()
            .putLong(LAST_TIME_KEY, System.currentTimeMillis())
            .putInt(LAST_STATUS_KEY, result.status)
            .putLong(LAST_FILES_KEY, result.files)
            .putLong(LAST_BYTES_KEY, result.bytes)
            .apply()
        Log.i(TAG, "automatic backup status ${result.status}, ${result.files} files")
        return result
    }

    private fun writeBackup(): Result {
        val folder = folder ?: return Result(NOT_SET_UP)
        val dataDirectory = preferences.getString(DATA_DIRECTORY_KEY, null)
            ?: return Result(NOT_SET_UP)
        val vault = EncryptionKeyVault(File(keyDirectory ?: return Result(NOT_SET_UP)))
        val passphrase = vault.loadBackupPassphrase() ?: return Result(NOT_SET_UP)
        val resolver = context.contentResolver
        val document = try {
            val parent = DocumentsContract.buildDocumentUriUsingTree(
                folder,
                DocumentsContract.getTreeDocumentId(folder),
            )
            val name = AUTO_PREFIX +
                SimpleDateFormat("yyyy-MM-dd-HHmmss", Locale.ROOT).format(Date()) + SUFFIX
            DocumentsContract.createDocument(resolver, parent, MIME_TYPE, name)
        } catch (error: Exception) {
            null
        }
        if (document == null) {
            passphrase.fill(0)
            return Result(FOLDER_UNAVAILABLE)
        }
        val summary = LongArray(2)
        val status = try {
            val descriptor = resolver.openFileDescriptor(document, "wt")
                ?: error("the backup file could not be opened")
            val key = vault.loadOrCreateKey()
            try {
                NativeCore.createBackup(
                    dataDirectory.toByteArray(Charsets.UTF_8),
                    DATABASE_NAME.toByteArray(Charsets.UTF_8),
                    descriptor.detachFd(),
                    passphrase,
                    key,
                    summary,
                )
            } finally {
                key.fill(0)
                descriptor.close()
            }
        } catch (error: Exception) {
            FOLDER_UNAVAILABLE
        } finally {
            passphrase.fill(0)
        }
        if (status != 0) {
            delete(document)
            return Result(status)
        }
        prune(folder)
        return Result(0, summary[0], summary[1])
    }

    /** Deletes the oldest automatic backups beyond `KEPT_BACKUPS`; the owner's own stay. */
    private fun prune(folder: Uri) {
        val children = DocumentsContract.buildChildDocumentsUriUsingTree(
            folder,
            DocumentsContract.getTreeDocumentId(folder),
        )
        val backups = mutableListOf<Pair<String, String>>()
        try {
            context.contentResolver.query(
                children,
                arrayOf(
                    DocumentsContract.Document.COLUMN_DOCUMENT_ID,
                    DocumentsContract.Document.COLUMN_DISPLAY_NAME,
                ),
                null,
                null,
                null,
            )?.use { cursor ->
                while (cursor.moveToNext()) {
                    val name = cursor.getString(1) ?: continue
                    if (name.startsWith(AUTO_PREFIX) && name.endsWith(SUFFIX)) {
                        backups.add(name to cursor.getString(0))
                    }
                }
            }
        } catch (error: Exception) {
            return
        }
        // The names hold the time, so sorting them sorts the backups oldest first.
        backups.sortBy { it.first }
        backups.dropLast(KEPT_BACKUPS).forEach { (_, id) ->
            delete(DocumentsContract.buildDocumentUriUsingTree(folder, id))
        }
    }

    private fun delete(document: Uri) {
        try {
            DocumentsContract.deleteDocument(context.contentResolver, document)
        } catch (error: Exception) {
            // The provider may refuse; the file holds nothing readable without the passphrase.
        }
    }

    /** The folder's name and, when known, the app that provides it (for example "Drive"). */
    private fun folderLabel(folder: Uri): String {
        val name = try {
            val document = DocumentsContract.buildDocumentUriUsingTree(
                folder,
                DocumentsContract.getTreeDocumentId(folder),
            )
            context.contentResolver.query(
                document,
                arrayOf(DocumentsContract.Document.COLUMN_DISPLAY_NAME),
                null,
                null,
                null,
            )?.use { cursor -> if (cursor.moveToFirst()) cursor.getString(0) else null }
        } catch (error: Exception) {
            null
        }
        val provider = folder.authority?.let { authority ->
            context.packageManager.resolveContentProvider(authority, 0)
                ?.applicationInfo?.loadLabel(context.packageManager)?.toString()
        }
        return listOfNotNull(name, provider?.let { "($it)" }).joinToString(" ").ifEmpty { "?" }
    }

    /** One run's outcome: the core's status, files and clear bytes. */
    data class Result(val status: Int, val files: Long = 0, val bytes: Long = 0) {
        fun toMap(): Map<String, Any> = mapOf("status" to status, "files" to files, "bytes" to bytes)
    }

    companion object {
        private const val TAG = "KoalinoAutoBackup"
        private const val JOB_ID = 8_702
        private const val PREFERENCES_NAME = "note_taker_automatic_backup"
        private const val FOLDER_KEY = "folder"
        private const val DATA_DIRECTORY_KEY = "data_directory"
        private const val KEY_DIRECTORY_KEY = "key_directory"
        private const val LAST_TIME_KEY = "last_time"
        private const val LAST_STATUS_KEY = "last_status"
        private const val LAST_FILES_KEY = "last_files"
        private const val LAST_BYTES_KEY = "last_bytes"
        private const val DATABASE_NAME = "note_taker.sqlite3"
        private const val MIME_TYPE = "application/octet-stream"
        private const val AUTO_PREFIX = "koalino-auto-"
        private const val SUFFIX = ".koalino-backup"
        private const val DAY_MS = 24L * 60 * 60 * 1000

        /** How many automatic backups stay in the folder. */
        const val KEPT_BACKUPS = 5

        /** Automatic backups are off, or their passphrase is gone. */
        const val NOT_SET_UP = 30

        /** The folder could not be written (removed, permission revoked, provider offline). */
        const val FOLDER_UNAVAILABLE = 31

        /** The access the folder picker grants and the app keeps. */
        const val FOLDER_ACCESS =
            Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION

        /**
         * Schedules the daily job: while charging, with storage not low; it survives reboots.
         *
         * Side effects: Replaces any scheduled run.
         */
        fun schedule(context: Context) {
            val job = JobInfo.Builder(
                JOB_ID,
                ComponentName(context, AutomaticBackupJob::class.java),
            )
                .setPeriodic(DAY_MS)
                .setRequiresCharging(true)
                .setRequiresStorageNotLow(true)
                .setPersisted(true)
                .build()
            val result = context.getSystemService(JobScheduler::class.java)?.schedule(job)
            if (result != JobScheduler.RESULT_SUCCESS) {
                Log.w(TAG, "automatic backups could not be scheduled")
            }
        }
    }
}

/** The daily job that runs `AutomaticBackup.runNow` on a worker thread. */
class AutomaticBackupJob : JobService() {
    /**
     * Starts the backup on a worker thread.
     * Outputs: true, as the work continues there. Side effects: See `AutomaticBackup.runNow`.
     */
    override fun onStartJob(params: JobParameters): Boolean {
        Thread({
            AutomaticBackup(this).runNow()
            jobFinished(params, false)
        }, "koalino-auto-backup").start()
        return true
    }

    /** Asks Android to retry when it stops the job; a partial file is deleted by the next run. */
    override fun onStopJob(params: JobParameters): Boolean = true
}
