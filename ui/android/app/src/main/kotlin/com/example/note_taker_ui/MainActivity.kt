// Hosts the Flutter UI, holds a Wi-Fi multicast lock while the app is visible so the Rust core's
// mDNS discovery receives other devices' announcements (Issue #43), and hands the database key
// from the Android Keystore to the Rust core over JNI when Dart asks on `note_taker/key_vault`.
// It also serves the app lock (`AppLock`) on `note_taker/app_lock` and the microphone permission
// (`MicrophonePermission`) on `note_taker/microphone`, and installs the shipped Whisper model
// (`WhisperModel`) on `note_taker/whisper_model`, and drives the recording foreground service
// (`RecordingService`) on `note_taker/recording_service`, reporting its notification's actions
// back to Dart.
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

    /** Answers `note_taker/microphone`; null until the engine is configured. */
    private var microphonePermission: MicrophonePermission? = null

    /** Runs Keystore work off the main thread. */
    private val keyVaultExecutor = Executors.newSingleThreadExecutor()

    /**
     * Registers the app-lock channel and `provideEncryptionKey(keyDirectory)`: unwraps the key, passes it to Rust through
     * `NativeCore`, wipes it, and answers null, so the key never reaches Dart. Failures answer
     * `keystore_unavailable` or `key_rejected`.
     */
    override fun configureFlutterEngine(flutterEngine: FlutterEngine) {
        super.configureFlutterEngine(flutterEngine)
        val appLock = AppLock(this)
        appLock.applyRecentsPreview()
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/app_lock")
            .setMethodCallHandler(appLock)
        val microphone = MicrophonePermission(this)
        microphonePermission = microphone
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/microphone")
            .setMethodCallHandler(microphone)
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/whisper_model")
            .setMethodCallHandler(WhisperModel(assets))
        val recordingChannel =
            MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/recording_service")
        recordingChannel.setMethodCallHandler { call, result ->
            if (call.method != "show") {
                result.notImplemented()
                return@setMethodCallHandler
            }
            val state = call.argument<String>("state")
            if (state == null) {
                result.error("invalid_argument", "state is required", null)
                return@setMethodCallHandler
            }
            try {
                RecordingService.show(this, state, call.argument<String>("title"))
                result.success(null)
            } catch (error: RuntimeException) {
                // For example a foreground service the system refuses to start.
                result.error("service_unavailable", error.message, null)
            }
        }
        RecordingService.stateListener = { state ->
            recordingChannel.invokeMethod("stateChanged", state)
        }
        val mainThread = Handler(Looper.getMainLooper())
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/key_vault")
            .setMethodCallHandler { call, result ->
                if (call.method != "provideEncryptionKey") {
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
                        val status = try {
                            NativeCore.provideEncryptionKey(key)
                        } finally {
                            key.fill(0)
                        }
                        mainThread.post {
                            if (status == 0) {
                                result.success(null)
                            } else {
                                result.error("key_rejected", "the core refused the key: $status", null)
                            }
                        }
                    } catch (error: Exception) {
                        // The message names the failing step only, never key material.
                        mainThread.post {
                            result.error("keystore_unavailable", error.message, null)
                        }
                    } catch (error: LinkageError) {
                        // The JNI library or method is missing; answer instead of hanging Dart.
                        mainThread.post {
                            result.error("key_rejected", error.message, null)
                        }
                    }
                }
            }
    }

    /** Stops reporting to an engine that is going away. */
    override fun cleanUpFlutterEngine(flutterEngine: FlutterEngine) {
        RecordingService.stateListener = null
        super.cleanUpFlutterEngine(flutterEngine)
    }

    /** Hands RECORD_AUDIO answers to `MicrophonePermission`. */
    override fun onRequestPermissionsResult(
        requestCode: Int,
        permissions: Array<out String>,
        grantResults: IntArray,
    ) {
        if (microphonePermission?.onRequestPermissionsResult(requestCode, grantResults) != true) {
            super.onRequestPermissionsResult(requestCode, permissions, grantResults)
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
