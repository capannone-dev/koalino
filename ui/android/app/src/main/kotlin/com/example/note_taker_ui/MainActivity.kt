// Hosts the Flutter UI and holds a Wi-Fi multicast lock while the app is visible, so the Rust
// core's mDNS discovery receives other devices' announcements (Issue #43).
package com.example.note_taker_ui

import android.content.Context
import android.net.wifi.WifiManager
import io.flutter.embedding.android.FlutterActivity

class MainActivity : FlutterActivity() {
    /** Lets multicast packets through Android's Wi-Fi filter; null until first started. */
    private var multicastLock: WifiManager.MulticastLock? = null

    /** Acquires the lock when the app becomes visible. Side effects: raises Wi-Fi power use. */
    override fun onStart() {
        super.onStart()
        val lock = multicastLock ?: (applicationContext.getSystemService(Context.WIFI_SERVICE)
            as WifiManager).createMulticastLock("note_taker_mdns").apply {
            setReferenceCounted(false)
        }
        multicastLock = lock
        lock.acquire()
    }

    /** Releases the lock when the app is hidden; background sync is a separate task (#47). */
    override fun onStop() {
        multicastLock?.takeIf { it.isHeld }?.release()
        super.onStop()
    }
}
