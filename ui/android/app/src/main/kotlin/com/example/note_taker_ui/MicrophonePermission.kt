// Serves the `note_taker/microphone` channel: `ensureGranted` answers whether the app holds
// RECORD_AUDIO, showing Android's dialog first when it does not (Issue #45). On Android 13 and
// later it asks for POST_NOTIFICATIONS in the same request, so the recording notification and
// its lock-screen controls are visible (Issue #47); refusing that one does not stop recording. The Rust core opens
// the microphone itself (cpal, AAudio) and would fail without it. Framework APIs only, so no
// AndroidX library is needed.
package com.example.note_taker_ui

import android.Manifest
import android.app.Activity
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Requests RECORD_AUDIO from `activity`, which forwards its permission results here. */
class MicrophonePermission(private val activity: Activity) : MethodChannel.MethodCallHandler {
    /** The `ensureGranted` call waiting for the dialog; one at a time. */
    private var pendingResult: MethodChannel.Result? = null

    /**
     * Answers `ensureGranted` with true or false.
     *
     * Side effects: Shows the system dialog unless the permission is granted (a permanent
     * refusal answers false without one). A second call while the dialog is open answers
     * `busy`. Every call answers exactly once.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (call.method != "ensureGranted") {
            result.notImplemented()
            return
        }
        // Before Android 6 the permission is granted at install time.
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M) {
            result.success(true)
            return
        }
        val missing = missingPermissions()
        if (missing.isEmpty()) {
            result.success(true)
            return
        }
        if (pendingResult != null) {
            result.error("busy", "a microphone permission request is already open", null)
            return
        }
        pendingResult = result
        activity.requestPermissions(missing.toTypedArray(), REQUEST_CODE)
    }

    /**
     * Completes the pending call with the dialog's answer.
     *
     * Inputs: The arguments of `Activity.onRequestPermissionsResult`.
     * Outputs: Whether the result belonged to this request.
     * Side effects: Answers the pending Dart call.
     */
    fun onRequestPermissionsResult(requestCode: Int, grantResults: IntArray): Boolean {
        if (requestCode != REQUEST_CODE) {
            return false
        }
        pendingResult?.success(isGranted(Manifest.permission.RECORD_AUDIO))
        pendingResult = null
        return true
    }

    /**
     * The permissions still to ask for: RECORD_AUDIO, and POST_NOTIFICATIONS on Android 13 and
     * later. Notifications alone are asked only once per install, so a refusal is not nagged.
     */
    private fun missingPermissions(): List<String> {
        val missing = mutableListOf<String>()
        if (!isGranted(Manifest.permission.RECORD_AUDIO)) {
            missing.add(Manifest.permission.RECORD_AUDIO)
        }
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU &&
            !isGranted(Manifest.permission.POST_NOTIFICATIONS) &&
            (missing.isNotEmpty() || !notificationsAsked())
        ) {
            missing.add(Manifest.permission.POST_NOTIFICATIONS)
            markNotificationsAsked()
        }
        return missing
    }

    private fun isGranted(permission: String): Boolean =
        activity.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED

    private fun notificationsAsked(): Boolean =
        activity.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)
            .getBoolean(NOTIFICATIONS_ASKED_KEY, false)

    private fun markNotificationsAsked() {
        activity.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE).edit()
            .putBoolean(NOTIFICATIONS_ASKED_KEY, true).apply()
    }

    private companion object {
        /** Identifies this request among the activity's permission results. */
        const val REQUEST_CODE = 4501
        const val PREFERENCES_NAME = "note_taker_permissions"
        const val NOTIFICATIONS_ASKED_KEY = "notifications_asked"
    }
}
