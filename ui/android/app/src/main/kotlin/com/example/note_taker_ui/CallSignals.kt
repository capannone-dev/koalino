// Tells Dart whether Android is feeding Koalino's recording silence because a call holds the
// microphone (#111). Answered on `note_taker/call_signals` as `silenced`.
package com.example.note_taker_ui

import android.content.Context
import android.media.AudioManager
import android.media.MediaRecorder
import android.os.Build
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Answers `silenced`. */
class CallSignals(private val context: Context) : MethodChannel.MethodCallHandler {
    /**
     * Inputs: `silenced`, with no arguments. Outputs: Whether a recording is silenced for a
     * call. Side effects: None.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (call.method == "silenced") {
            result.success(isRecordingSilenced(context))
        } else {
            result.notImplemented()
        }
    }

    companion object {
        /**
         * Whether Android is feeding a non-call capture silence (#111): its concurrent-capture
         * policy gives the microphone to a `VOICE_COMMUNICATION` capture and silences the
         * others. Other apps' captures are anonymized, so this cannot tell Koalino's own from
         * another app's; callers ask only while Koalino records.
         *
         * Outputs: true while such a capture is silenced; false otherwise or before Android 10,
         * which does not report silencing. Side effects: None.
         */
        fun isRecordingSilenced(context: Context): Boolean {
            if (Build.VERSION.SDK_INT < Build.VERSION_CODES.Q) {
                return false
            }
            val audio = context.getSystemService(AudioManager::class.java) ?: return false
            return audio.activeRecordingConfigurations.any {
                it.isClientSilenced &&
                    it.clientAudioSource != MediaRecorder.AudioSource.VOICE_COMMUNICATION
            }
        }
    }
}
