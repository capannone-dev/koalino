// Serves the `note_taker/microphone` channel: `ensureGranted` answers whether the app holds
// RECORD_AUDIO, showing Android's dialog first when it does not (Issue #45). The Rust core opens
// the microphone itself (cpal, AAudio) and would fail without it. Framework APIs only, so no
// AndroidX library is needed.
package com.example.note_taker_ui

import android.Manifest
import android.app.Activity
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
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.M || isGranted()) {
            result.success(true)
            return
        }
        if (pendingResult != null) {
            result.error("busy", "a microphone permission request is already open", null)
            return
        }
        pendingResult = result
        activity.requestPermissions(arrayOf(Manifest.permission.RECORD_AUDIO), REQUEST_CODE)
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
        val granted = grantResults.firstOrNull() == PackageManager.PERMISSION_GRANTED
        pendingResult?.success(granted)
        pendingResult = null
        return true
    }

    private fun isGranted(): Boolean =
        activity.checkSelfPermission(Manifest.permission.RECORD_AUDIO) ==
            PackageManager.PERMISSION_GRANTED

    private companion object {
        /** Identifies this request among the activity's permission results. */
        const val REQUEST_CODE = 4501
    }
}
