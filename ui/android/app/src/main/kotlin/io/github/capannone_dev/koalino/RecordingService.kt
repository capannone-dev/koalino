// Keeps a recording alive while the app is in the background or the screen is locked (Issue
// #47). It is a foreground service of type `microphone`, which Android requires for capture
// outside the visible app; the Rust core does the capture itself. Its ongoing notification,
// also shown on the lock screen, offers Pause, Resume, and Stop. The actions call the core over
// JNI (`NativeCore`), so they work without opening or unlocking the app, and every change is
// reported to Dart through `stateListener`. Dart starts, updates, and ends the service with
// `show` as its own recorder changes. The lock-screen version of the notification hides the
// meeting title. While Android feeds the recording silence because a call holds the microphone,
// the notification says so (#111), with the text Dart sent. Framework APIs only, no AndroidX.
package io.github.capannone_dev.koalino

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
import android.media.AudioManager
import android.media.AudioRecordingConfiguration
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import java.util.concurrent.Executors

class RecordingService : Service() {
    /** Runs the blocking core calls (stop waits for transcription) off the main thread. */
    private val coreExecutor = Executors.newSingleThreadExecutor()
    private val mainThread = Handler(Looper.getMainLooper())

    /** What the notification shows: `recording`, `paused`, or `stopping`. */
    private var state = STATE_RECORDING

    /** The meeting title, or null when untitled; shown only when the device is unlocked. */
    private var title: String? = null

    /** Dart's localized sentence for a silenced recording, or null before Dart sent one. */
    private var silencedText: String? = null

    /** Whether Android is feeding the recording silence for a call (#111). */
    private var silenced = false

    /** Follows capture changes, so the notification says when a call silences the recording. */
    private val recordingCallback = object : AudioManager.AudioRecordingCallback() {
        override fun onRecordingConfigChanged(configs: MutableList<AudioRecordingConfiguration>) {
            refreshSilenced()
        }
    }

    override fun onBind(intent: Intent?): IBinder? = null

    /** Side effects: Registers the capture callback on Android 10 and later. */
    override fun onCreate() {
        super.onCreate()
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            getSystemService(AudioManager::class.java)
                ?.registerAudioRecordingCallback(recordingCallback, mainThread)
        }
    }

    /** Updates the notification when a call starts or stops silencing the recording. */
    private fun refreshSilenced() {
        val nowSilenced = state == STATE_RECORDING && CallSignals.isRecordingSilenced(this)
        if (nowSilenced != silenced) {
            silenced = nowSilenced
            post()
        }
    }

    /**
     * Handles `show` from Dart and the notification's actions.
     *
     * Side effects: Posts the notification as a foreground service; actions call the Rust core
     * on a background thread and report the new state. A restart without an intent (after the
     * process was killed) ends the service, because the recording ended with the process.
     */
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        when (intent?.action) {
            ACTION_SHOW -> {
                state = intent.getStringExtra(EXTRA_STATE) ?: STATE_RECORDING
                title = intent.getStringExtra(EXTRA_TITLE)
                silencedText = intent.getStringExtra(EXTRA_SILENCED_TEXT) ?: silencedText
                silenced = state == STATE_RECORDING && CallSignals.isRecordingSilenced(this)
                post()
            }
            ACTION_PAUSE -> runCore(STATE_PAUSED) { NativeCore.pauseRecording() }
            ACTION_RESUME -> runCore(STATE_RECORDING) { NativeCore.resumeRecording() }
            ACTION_STOP -> stopFromNotification()
            else -> stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            getSystemService(AudioManager::class.java)
                ?.unregisterAudioRecordingCallback(recordingCallback)
        }
        coreExecutor.shutdown()
        super.onDestroy()
    }

    /** Applies a pause or resume; on success shows and reports `newState`. */
    private fun runCore(newState: String, call: () -> Int) {
        if (state == STATE_STOPPING) {
            return
        }
        coreExecutor.execute {
            val status = call()
            mainThread.post {
                if (status == 0) {
                    state = newState
                    post()
                    stateListener?.invoke(newState)
                }
            }
        }
    }

    /** Shows "finishing", waits for the core to transcribe the rest, then ends the service. */
    private fun stopFromNotification() {
        if (state == STATE_STOPPING) {
            return
        }
        val stateBeforeStop = state
        state = STATE_STOPPING
        post()
        stateListener?.invoke(STATE_STOPPING)
        coreExecutor.execute {
            val status = NativeCore.stopRecording()
            mainThread.post {
                if (status == 0) {
                    stateListener?.invoke(STATE_IDLE)
                    end()
                } else {
                    state = stateBeforeStop
                    post()
                    stateListener?.invoke(stateBeforeStop)
                }
            }
        }
    }

    /** Leaves the foreground and stops. */
    private fun end() {
        stopForeground(STOP_FOREGROUND_REMOVE)
        stopSelf()
    }

    /** Posts the notification for `state` as this foreground service's notification. */
    private fun post() {
        val notification = buildNotification(includeTitle = true)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
            startForeground(
                NOTIFICATION_ID,
                notification,
                ServiceInfo.FOREGROUND_SERVICE_TYPE_MICROPHONE,
            )
        } else {
            startForeground(NOTIFICATION_ID, notification)
        }
    }

    /**
     * Builds the notification; the lock-screen (public) version omits the title.
     *
     * Inputs: Whether to include the meeting title.
     * Outputs: The notification with the actions that fit `state`.
     */
    private fun buildNotification(includeTitle: Boolean): Notification {
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            ensureChannel()
            Notification.Builder(this, CHANNEL_ID)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(this)
        }
        val heading = when (state) {
            STATE_PAUSED -> "Recording paused"
            STATE_STOPPING -> "Finishing transcription…"
            else -> "Recording"
        }
        builder
            .setSmallIcon(android.R.drawable.ic_btn_speak_now)
            .setContentTitle(heading)
            .setOngoing(true)
            .setOnlyAlertOnce(true)
            .setShowWhen(false)
            .setCategory(Notification.CATEGORY_SERVICE)
            .setContentIntent(openAppIntent())
            .setVisibility(
                if (includeTitle) Notification.VISIBILITY_PRIVATE else Notification.VISIBILITY_PUBLIC,
            )
        val meetingTitle = title
        if (includeTitle && !meetingTitle.isNullOrEmpty()) {
            builder.setContentText(meetingTitle)
        }
        // The warning names no meeting, so the lock screen shows it too (#111).
        val warning = silencedText
        if (silenced && state == STATE_RECORDING && warning != null) {
            builder.setContentText(warning).setStyle(Notification.BigTextStyle().bigText(warning))
        }
        if (includeTitle) {
            builder.setPublicVersion(buildNotification(includeTitle = false))
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            builder.setForegroundServiceBehavior(Notification.FOREGROUND_SERVICE_IMMEDIATE)
        }
        when (state) {
            STATE_RECORDING -> {
                builder.addAction(action(android.R.drawable.ic_media_pause, "Pause", ACTION_PAUSE))
                builder.addAction(action(android.R.drawable.ic_delete, "Stop", ACTION_STOP))
            }
            STATE_PAUSED -> {
                builder.addAction(action(android.R.drawable.ic_media_play, "Resume", ACTION_RESUME))
                builder.addAction(action(android.R.drawable.ic_delete, "Stop", ACTION_STOP))
            }
        }
        return builder.build()
    }

    /** A notification action that sends `intentAction` back to this service. */
    private fun action(icon: Int, label: String, intentAction: String): Notification.Action {
        val intent = Intent(this, RecordingService::class.java).setAction(intentAction)
        val pendingIntent = PendingIntent.getService(
            this,
            intentAction.hashCode(),
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        return Notification.Action.Builder(
            Icon.createWithResource(this, icon),
            label,
            pendingIntent,
        ).build()
    }

    /** Opens the app on a tap on the notification. */
    private fun openAppIntent(): PendingIntent {
        val intent = Intent(this, MainActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP)
        return PendingIntent.getActivity(
            this,
            0,
            intent,
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
    }

    /** Creates the silent, low-importance channel once (Android 8 and later). */
    private fun ensureChannel() {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
            return
        }
        val manager = getSystemService(NotificationManager::class.java)
        if (manager.getNotificationChannel(CHANNEL_ID) == null) {
            val channel = NotificationChannel(
                CHANNEL_ID,
                "Recording",
                NotificationManager.IMPORTANCE_LOW,
            )
            channel.description = "Shows an ongoing recording with Pause, Resume, and Stop"
            channel.lockscreenVisibility = Notification.VISIBILITY_PUBLIC
            manager.createNotificationChannel(channel)
        }
    }

    companion object {
        const val STATE_RECORDING = "recording"
        const val STATE_PAUSED = "paused"
        const val STATE_STOPPING = "stopping"
        const val STATE_IDLE = "idle"

        private const val ACTION_SHOW = "io.github.capannone_dev.koalino.recording.SHOW"
        private const val ACTION_PAUSE = "io.github.capannone_dev.koalino.recording.PAUSE"
        private const val ACTION_RESUME = "io.github.capannone_dev.koalino.recording.RESUME"
        private const val ACTION_STOP = "io.github.capannone_dev.koalino.recording.STOP"
        private const val EXTRA_STATE = "state"
        private const val EXTRA_TITLE = "title"
        private const val EXTRA_SILENCED_TEXT = "silencedText"
        private const val CHANNEL_ID = "recording"
        private const val NOTIFICATION_ID = 4701

        /** Receives states reached from the notification, on the main thread; set by MainActivity. */
        @Volatile
        var stateListener: ((String) -> Unit)? = null

        /**
         * Starts or updates the service for `state`, or ends it for `idle`.
         *
         * Inputs: A state name from Dart, the meeting title (null when untitled), and the
         * sentence to show while a call silences the recording.
         * Side effects: Starts a foreground service; call while the app is visible (Android 12
         * and later refuse to start one from the background, which Dart never does).
         */
        fun show(context: Context, state: String, title: String?, silencedText: String? = null) {
            val intent = Intent(context, RecordingService::class.java)
            if (state == STATE_IDLE) {
                context.stopService(intent)
                return
            }
            intent.setAction(ACTION_SHOW)
                .putExtra(EXTRA_STATE, state)
                .putExtra(EXTRA_TITLE, title)
                .putExtra(EXTRA_SILENCED_TEXT, silencedText)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }
    }
}
