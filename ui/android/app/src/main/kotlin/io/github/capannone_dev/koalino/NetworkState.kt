// Tells the app whether the phone is on an unmetered network such as Wi-Fi (#189), so Automatic's
// model is downloaded by itself without spending mobile data.
package io.github.capannone_dev.koalino

import android.content.Context
import android.net.ConnectivityManager
import android.net.NetworkCapabilities
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Answers `isUnmetered` on the `note_taker/network` channel. */
class NetworkState(private val context: Context) : MethodChannel.MethodCallHandler {
    /**
     * Inputs: `isUnmetered`, with no arguments. Outputs: `true` on a connected network that is not
     * metered (Wi-Fi, Ethernet), `false` on mobile data, a metered hotspot or no network.
     * Side effects: None.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        if (call.method != "isUnmetered") {
            result.notImplemented()
            return
        }
        result.success(isUnmetered())
    }

    private fun isUnmetered(): Boolean {
        val manager = context.getSystemService(ConnectivityManager::class.java) ?: return false
        val network = manager.activeNetwork ?: return false
        val capabilities = manager.getNetworkCapabilities(network) ?: return false
        return capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_INTERNET) &&
            capabilities.hasCapability(NetworkCapabilities.NET_CAPABILITY_NOT_METERED)
    }
}
