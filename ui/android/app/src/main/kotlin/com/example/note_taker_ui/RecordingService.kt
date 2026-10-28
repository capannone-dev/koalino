// Keeps a recording alive while the app is in the background or the screen is locked (Issue
// #47). It is a foreground service of type `microphone`, which Android requires for capture
// outside the visible app; the Rust core does the capture itself. Its ongoing notification,
// also shown on the lock screen, offers Pause, Resume, and Stop. The actions call the core over
// JNI (`NativeCore`), so they work without opening or unlocking the app, and every change is
// reported to Dart through `stateListener`. Dart starts, updates, and ends the service with
// `show` as its own recorder changes. The lock-screen version of the notification hides the
// meeting title. Framework APIs only, no AndroidX.
package com.example.note_taker_ui

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.graphics.drawable.Icon
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

    override fun onBind(intent: Intent?): IBinder? = null

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

        private const val ACTION_SHOW = "com.example.note_taker_ui.recording.SHOW"
        private const val ACTION_PAUSE = "com.example.note_taker_ui.recording.PAUSE"
        private const val ACTION_RESUME = "com.example.note_taker_ui.recording.RESUME"
        private const val ACTION_STOP = "com.example.note_taker_ui.recording.STOP"
        private const val EXTRA_STATE = "state"
        private const val EXTRA_TITLE = "title"
        private const val CHANNEL_ID = "recording"
        private const val NOTIFICATION_ID = 4701

        /** Receives states reached from the notification, on the main thread; set by MainActivity. */
        @Volatile
        var stateListener: ((String) -> Unit)? = null

        /**
         * Starts or updates the service for `state`, or ends it for `idle`.
         *
         * Inputs: A state name from Dart and the meeting title (null when untitled).
         * Side effects: Starts a foreground service; call while the app is visible (Android 12
         * and later refuse to start one from the background, which Dart never does).
         */
        fun show(context: Context, state: String, title: String?) {
            val intent = Intent(context, RecordingService::class.java)
            if (state == STATE_IDLE) {
                context.stopService(intent)
                return
            }
            intent.setAction(ACTION_SHOW)
                .putExtra(EXTRA_STATE, state)
                .putExtra(EXTRA_TITLE, title)
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
                context.startForegroundService(intent)
            } else {
                context.startService(intent)
            }
        }
    }
}
