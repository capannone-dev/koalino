// Hosts the Flutter UI, holds a Wi-Fi multicast lock while the app is visible so the Rust core's
// mDNS discovery receives other devices' announcements (Issue #43), and serves the database key
// from the Android Keystore over the `note_taker/key_vault` channel.
package com.example.note_taker_ui

import android.content.Context
import android.net.wifi.WifiManager
import android.os.Handler
import android.os.Looper
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.util.concurrent.Executors

class MainActivity : FlutterActivity() {
    /** Lets multicast packets through Android's Wi-Fi filter; null until first started. */
    private var multicastLock: WifiManager.MulticastLock? = null

    /** Runs Keystore work off the main thread. */
    private val keyVaultExecutor = Executors.newSingleThreadExecutor()

    /** Registers `loadEncryptionKey(keyDirectory)`, which answers the 32-byte key or an error. */
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        val mainThread = Handler(Looper.getMainLooper())
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/key_vault")
            .setMethodCallHandler { call, result ->
                if (call.method != "loadEncryptionKey") {
                    result.notImplemented()
                    return@setMethodCallHandler
                }
                val keyDirectory = call.argument<String>("keyDirectory")
                if (keyDirectory == null) {
                    result.error("invalid_argument", "keyDirectory is required", null)
                    return@setMethodCallHandler
                }
                keyVaultExecutor.execute {
                    try {
                        val key = EncryptionKeyVault(File(keyDirectory)).loadOrCreateKey()
                        mainThread.post {
                            // The codec copies the bytes when `success` runs; then wipe ours.
                            result.success(key)
                            key.fill(0)
                        }
                    } catch (error: Exception) {
                        // The message names the failing step only, never key material.
                        mainThread.post {
                            result.error("keystore_unavailable", error.message, null)
                        }
                    }
                }
            }
    }

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
