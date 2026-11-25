// Runs the after-meeting transcript pass from a JobScheduler job while the phone charges, so a
// meeting's speech gets its pass even when Koalino is closed and the screen is locked (#87).
//
// Android does not deliver "power connected" to closed apps, but JobScheduler starts a job whose
// charging constraint becomes true. The job opens the Rust core without Flutter: the app stores
// its last core configuration (paths and device name, no secrets), and the database key is
// unwrapped from the Keystore exactly as at app start. That key needs no user authentication, so
// the job works with the screen locked and with Koalino's app lock on: the app lock gates the UI
// only. Before the first unlock after a reboot, app storage is unavailable and Android does not
// start the job.
package io.github.capannone_dev.koalino

import android.app.job.JobInfo
import android.app.job.JobParameters
import android.app.job.JobScheduler
import android.app.job.JobService
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import android.util.Log
import java.io.File
import java.util.concurrent.atomic.AtomicBoolean

/** The job that runs waiting after-meeting passes while charging. */
class WaitingPassJob : JobService() {
    /** Set once Android stops the job; the worker thread then ends after the window in progress. */
    private val stopped = AtomicBoolean(false)

    /**
     * Starts the pass on a worker thread.
     *
     * Inputs: The job's parameters. Outputs: true, as the work continues on the thread.
     * Side effects: See `runPass`.
     */
    override fun onStartJob(params: JobParameters): Boolean {
        stopped.set(false)
        Thread({ runPass(params) }, "koalino-waiting-pass").start()
        return true
    }

    /**
     * Ends the pass when Android stops the job (charger removed, or its time is up).
     *
     * Outputs: true, so Android runs the job again when it may.
     * Side effects: Reports "not charging" to the core on a new thread, which ends the round
     * after the window in progress; the pass starts over at the next run.
     */
    override fun onStopJob(params: JobParameters): Boolean {
        stopped.set(true)
        val reason = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) params.stopReason else -1
        Log.i(TAG, "stopped by Android (reason $reason)")
        Thread({ NativeCore.setCharging(false) }, "koalino-waiting-pass-stop").start()
        return true
    }

    /**
     * Opens the core, runs rounds of the pass until no speech awaits it, and finishes the job.
     *
     * Inputs: The job's parameters, for `jobFinished`.
     * Side effects: Unwraps the database key, configures and opens the core (database, P2P),
     * loads the model and re-transcribes meetings; schedules a later run when meetings wait for
     * another device. Logs counts and statuses only.
     */
    private fun runPass(params: JobParameters) {
        val started = System.currentTimeMillis()
        val configuration = BackgroundCoreConfiguration(this)
        val status = openCore(configuration)
        if (status != 0) {
            Log.i(TAG, "the core could not be opened (status $status)")
            jobFinished(params, false)
            return
        }
        NativeCore.setCharging(isCharging())
        var awaiting = NativeCore.speechAwaitingPass()
        Log.i(TAG, "started; $awaiting meeting(s) await the pass")
        while (awaiting > 0 && !stopped.get()) {
            val round = NativeCore.runWaitingPass(ROUND_WAIT_MS)
            val left = NativeCore.speechAwaitingPass()
            val seconds = (System.currentTimeMillis() - started) / 1000
            Log.i(TAG, "round status $round after ${seconds}s; $left meeting(s) await the pass")
            if (round == 0 && left >= awaiting) {
                // The round left them waiting (for another device's recording): try again later.
                if (!stopped.get()) {
                    schedule(this, RETRY_DELAY_MS)
                    jobFinished(params, false)
                }
                return
            }
            awaiting = left
        }
        if (!stopped.get()) {
            Log.i(TAG, "finished after ${(System.currentTimeMillis() - started) / 1000}s")
            jobFinished(params, false)
        }
    }

    /**
     * Configures the core and opens it with the Keystore key, unless the app already did.
     *
     * Outputs: 0 when open; 1 without a stored configuration or key; otherwise the core's status.
     * Side effects: Hands the key to the core over JNI and wipes the copy; never creates a key.
     */
    private fun openCore(configuration: BackgroundCoreConfiguration): Int {
        val configurationJson = configuration.configurationJson ?: return 1
        val keyDirectory = configuration.keyDirectory ?: return 1
        if (!File(keyDirectory, "encryption.key.wrapped").exists()) {
            return 1
        }
        // 2 means this process's app already configured the core with the same settings.
        val configured = NativeCore.configure(configurationJson.toByteArray(Charsets.UTF_8))
        if (configured != 0 && configured != 2) {
            return configured
        }
        val key = EncryptionKeyVault(File(keyDirectory)).loadOrCreateKey()
        try {
            val provided = NativeCore.provideEncryptionKey(key)
            if (provided != 0) {
                return provided
            }
        } finally {
            key.fill(0)
        }
        return NativeCore.startRuntime()
    }

    /** Reads the sticky battery broadcast. Outputs: Whether a charger is connected. */
    private fun isCharging(): Boolean {
        val battery = registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        return (battery?.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) ?: 0) != 0
    }

    companion object {
        private const val TAG = "KoalinoWaitingPass"
        private const val JOB_ID = 8_701
        /** How long one call waits for a round before checking again. */
        private const val ROUND_WAIT_MS = 10 * 60 * 1000
        /** When to look again at meetings that wait for another device's recording. */
        private const val RETRY_DELAY_MS = 30L * 60 * 1000

        /**
         * Asks Android to run the job the next time the phone charges, after `delayMs`.
         *
         * Inputs: Any context; the delay in milliseconds (0 for as soon as charging).
         * Outputs: None. Side effects: Replaces any scheduled run; the job survives a reboot.
         * When no speech awaits the pass, the run ends at once.
         */
        fun schedule(context: Context, delayMs: Long = 0) {
            val job = JobInfo.Builder(
                JOB_ID,
                ComponentName(context, WaitingPassJob::class.java),
            )
                .setRequiresCharging(true)
                .setMinimumLatency(delayMs)
                .setPersisted(true)
                .build()
            val scheduler = context.getSystemService(JobScheduler::class.java)
            val result = scheduler?.schedule(job) ?: JobScheduler.RESULT_FAILURE
            if (result != JobScheduler.RESULT_SUCCESS) {
                Log.w(TAG, "the waiting pass could not be scheduled")
            }
        }
    }
}

/**
 * The core configuration the app last used, kept for the background job (#87).
 *
 * Holds the JSON passed to `note_taker_configure` (the data directory, model paths, device name
 * and peers; no secrets) and the key directory, in private SharedPreferences.
 */
class BackgroundCoreConfiguration(context: Context) {
    private val preferences =
        context.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)

    /** The stored configuration JSON, or null before the app first started. */
    val configurationJson: String?
        get() = preferences.getString(CONFIGURATION_KEY, null)

    /** The stored key directory, or null before the app first started. */
    val keyDirectory: String?
        get() = preferences.getString(KEY_DIRECTORY_KEY, null)

    /**
     * Stores what the app passed to the core.
     *
     * Inputs: The configuration JSON and the key directory. Side effects: Writes the preferences.
     */
    fun remember(configurationJson: String, keyDirectory: String) {
        preferences.edit()
            .putString(CONFIGURATION_KEY, configurationJson)
            .putString(KEY_DIRECTORY_KEY, keyDirectory)
            .apply()
    }

    private companion object {
        const val PREFERENCES_NAME = "note_taker_background_core"
        const val CONFIGURATION_KEY = "configuration_json"
        const val KEY_DIRECTORY_KEY = "key_directory"
    }
}
