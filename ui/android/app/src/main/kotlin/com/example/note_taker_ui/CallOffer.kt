// Shows the offer to record a call as a notification (#111), so it is seen while the call app is
// in front. Record opens Koalino (a recording's foreground service can only start while the app
// is visible) and answers `record` on `note_taker/call_offer`; Not now, or swiping the
// notification away, answers `dismissed`. Dart calls `show` with the app's name and `withdraw`
// when the offer no longer stands. The notification expires after two minutes.
package com.example.note_taker_ui

import android.app.Activity
import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.graphics.drawable.Icon
import android.os.Build
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Posts and withdraws the call offer, and reports its answer to Dart. */
class CallOffer(private val activity: Activity) : MethodChannel.MethodCallHandler {
    /** The channel answers go to; set by MainActivity, null once the engine is gone. */
    var channel: MethodChannel? = null

    private val dismissReceiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            cancel()
            channel?.invokeMethod("answer", "dismissed")
        }
    }

    /**
     * Starts listening for Not now and swipes. Side effects: Registers a receiver that only this
     * app can reach.
     */
    fun start() {
        val filter = IntentFilter(ACTION_DISMISS)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            activity.registerReceiver(dismissReceiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            activity.registerReceiver(dismissReceiver, filter)
        }
    }

    /** Stops listening. Side effects: Unregisters the receiver and removes the notification. */
    fun stop() {
        try {
            activity.unregisterReceiver(dismissReceiver)
        } catch (error: IllegalArgumentException) {
            // Never registered; nothing to undo.
        }
        cancel()
    }

    /**
     * Inputs: `show` with `{app}`, or `withdraw`. Outputs: `show` answers whether the
     * notification was posted (false when notifications are off). Side effects: Posts or removes
     * the notification.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "show" -> result.success(show(call.argument<String>("app") ?: "A call app"))
            "withdraw" -> {
                cancel()
                result.success(null)
            }
            else -> result.notImplemented()
        }
    }

    /**
     * Answers `record` when the activity was opened from the notification's Record.
     *
     * Inputs: The intent that opened or reached the activity. Outputs: Whether it was the offer's.
     * Side effects: Removes the notification and tells Dart.
     */
    fun receive(intent: Intent?): Boolean {
        if (intent?.action != ACTION_RECORD) {
            return false
        }
        cancel()
        channel?.invokeMethod("answer", "record")
        return true
    }

    /** Posts the offer. Outputs: false when the app may not post notifications. */
    private fun show(app: String): Boolean {
        val manager = activity.getSystemService(NotificationManager::class.java) ?: return false
        if (!manager.areNotificationsEnabled()) {
            return false
        }
        ensureChannel(manager)
        val record = PendingIntent.getActivity(
            activity,
            REQUEST_RECORD,
            Intent(activity, MainActivity::class.java)
                .setAction(ACTION_RECORD)
                .addFlags(Intent.FLAG_ACTIVITY_SINGLE_TOP or Intent.FLAG_ACTIVITY_CLEAR_TOP),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val dismiss = PendingIntent.getBroadcast(
            activity,
            REQUEST_DISMISS,
            Intent(ACTION_DISMISS).setPackage(activity.packageName),
            PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE,
        )
        val builder = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
            Notification.Builder(activity, CHANNEL_ID).setTimeoutAfter(EXPIRE_MS)
        } else {
            @Suppress("DEPRECATION")
            Notification.Builder(activity).setPriority(Notification.PRIORITY_HIGH)
        }
        val icon = Icon.createWithResource(activity, android.R.drawable.ic_btn_speak_now)
        val notification = builder
            .setSmallIcon(icon)
            .setContentTitle("$app is using the microphone")
            .setContentText("Record this call?")
            .setCategory(Notification.CATEGORY_RECOMMENDATION)
            .setAutoCancel(true)
            .setContentIntent(record)
            .setDeleteIntent(dismiss)
            .addAction(Notification.Action.Builder(icon, "Record", record).build())
            .addAction(Notification.Action.Builder(icon, "Not now", dismiss).build())
            .build()
        manager.notify(NOTIFICATION_ID, notification)
        return true
    }

    /** Removes the offer, if shown. */
    private fun cancel() {
        activity.getSystemService(NotificationManager::class.java)?.cancel(NOTIFICATION_ID)
    }

    /** Creates the channel once: high importance, so the offer appears over the call app. */
    private fun ensureChannel(manager: NotificationManager) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O ||
            manager.getNotificationChannel(CHANNEL_ID) != null
        ) {
            return
        }
        val channel = NotificationChannel(CHANNEL_ID, "Calls", NotificationManager.IMPORTANCE_HIGH)
        channel.description = "Offers to record when a call starts"
        channel.setSound(null, null)
        manager.createNotificationChannel(channel)
    }

    companion object {
        private const val ACTION_RECORD = "com.example.note_taker_ui.call.RECORD"
        private const val ACTION_DISMISS = "com.example.note_taker_ui.call.DISMISS"
        private const val CHANNEL_ID = "call_offer"
        private const val NOTIFICATION_ID = 4702
        private const val REQUEST_RECORD = 4702
        private const val REQUEST_DISMISS = 4703
        private const val EXPIRE_MS = 120_000L
    }
}
