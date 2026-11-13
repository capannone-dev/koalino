// Tells Dart whether another app holds the microphone for a call (#111). Android does not say
// which app captures the microphone (the recording configurations hide the client's package from
// ordinary apps), so a call shows as one unnamed user, `android:voice-communication`, when either:
// - the audio mode is `MODE_IN_COMMUNICATION`, which VoIP apps (Teams, Zoom, Meet in a browser,
//   WhatsApp) set for the call; or
// - an active recording uses the `VOICE_COMMUNICATION` source.
// Koalino's own recorder uses neither, so it never counts. Answered on `note_taker/call_signals`.
package com.example.note_taker_ui

import android.content.Context
import android.media.AudioManager
import android.media.MediaRecorder
import android.os.Build
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Answers `users` with the call it sees, if any. */
class CallSignals(private val context: Context) : MethodChannel.MethodCallHandler {
    /**
     * Inputs: `users`, with no arguments. Outputs: A list of `{process, name}` maps, empty when no
     * call holds the microphone. Side effects: None.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (call.method != "users") {
            result.notImplemented()
            return
        }
        val users = if (inCall()) {
            listOf(mapOf("process" to VOICE_CALL, "name" to ""))
        } else {
            emptyList()
        }
        result.success(users)
    }

    /** Whether the audio mode or a capture says a voice call is in progress. */
    private fun inCall(): Boolean {
        val audio = context.getSystemService(AudioManager::class.java) ?: return false
        if (audio.mode == AudioManager.MODE_IN_COMMUNICATION) {
            return true
        }
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.N) {
            return false
        }
        return audio.activeRecordingConfigurations.any {
            it.clientAudioSource == MediaRecorder.AudioSource.VOICE_COMMUNICATION
        }
    }

    companion object {
        /** The process name the core knows as an unnamed call app (`domain::call_watch`). */
        const val VOICE_CALL = "android:voice-communication"
    }
}
