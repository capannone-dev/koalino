// Receives the pairing invite links a phone's camera opens (Issue #122). The QR code another
// device shows holds a `note-taker-invite:` link; the manifest makes Koalino the app for that
// scheme, and this class keeps the link until Dart takes it on `note_taker/pairing_links`. The
// link is untrusted text: Dart asks the owner first, and the Rust core checks the invite.
package io.github.capannone_dev.koalino

import android.content.Intent
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Keeps the latest invite link and answers `takePending`. */
class PairingLinks : MethodChannel.MethodCallHandler {
    /** The latest link Dart has not taken yet, or null. Main thread only. */
    private var pendingLink: String? = null

    /** The channel to tell Dart a link arrived while it runs; null until the engine exists. */
    var channel: MethodChannel? = null

    /**
     * Keeps the link `intent` opens, if it is an invite link.
     *
     * Inputs: The intent that started or reached the activity.
     * Outputs: Whether it was an invite link (the caller then clears it so it is not used twice).
     * Side effects: Stores the link and calls `linkArrived` on the channel.
     */
    fun receive(intent: Intent?): Boolean {
        if (intent?.action != Intent.ACTION_VIEW || intent.data?.scheme != SCHEME) {
            return false
        }
        val link = intent.dataString ?: return false
        if (link.length > MAX_LINK_CHARS) {
            return false
        }
        pendingLink = link
        channel?.invokeMethod("linkArrived", null)
        return true
    }

    /** `takePending`: the pending link, or null; it is then no longer pending. */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "takePending" -> {
                result.success(pendingLink)
                pendingLink = null
            }
            else -> result.notImplemented()
        }
    }

    private companion object {
        /** The scheme of invite links, as the core writes them. */
        const val SCHEME = "note-taker-invite"

        /** Longest link kept; real invites are a few hundred characters. */
        const val MAX_LINK_CHARS = 4096
    }
}
