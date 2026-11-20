// Hosts the Flutter UI, holds a Wi-Fi multicast lock while the app is visible so the Rust core's
// mDNS discovery receives other devices' announcements (Issue #43), and hands the database key
// from the Android Keystore to the Rust core over JNI when Dart asks on `note_taker/key_vault`.
// It also serves the app lock (`AppLock`) on `note_taker/app_lock` and the microphone permission
// (`MicrophonePermission`) on `note_taker/microphone`, and installs the shipped Whisper model
// (`WhisperModel`) on `note_taker/whisper_model`, and drives the recording foreground service
// (`RecordingService`) on `note_taker/recording_service`, reporting its notification's actions
// back to Dart. Exported logs are shared through the share sheet on `note_taker/share`
// (`SharedFiles`, Issue #46). Files and e-mails shared to the app from others are copied by
// `IncomingShares` and offered to Dart on `note_taker/incoming_shares` (Issue #85).
// `ChargingReporter` tells the core when the phone charges, so the after-meeting pass can run (#87).
// `CallSignals` (`note_taker/call_signals`) tells Dart when a call silences a recording (#111).
// The phone does not offer to record calls; the call offer's old notification channel is removed.
// `NpuDownload` (`note_taker/npu`) fetches the NPU's Whisper files on request and points the
// NPU's library search at them (#75). `PairingLinks` (`note_taker/pairing_links`) hands Dart
// the pairing invite links the camera opens from another device's QR code (#122).
// `note_taker/background_core` stores the core settings for `WaitingPassJob`, which runs a
// waiting after-meeting pass while the phone charges with the app closed, and asks once for the
// battery exemption Power saving requires; leaving the app schedules that job (#87).
package com.example.note_taker_ui

import android.content.ActivityNotFoundException
import android.content.Context
import android.content.Intent
import android.net.Uri
import android.net.wifi.WifiManager
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.PowerManager
import android.provider.Settings
import io.flutter.embedding.android.FlutterActivity
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.util.concurrent.Executors

class MainActivity : FlutterActivity() {
    /** Lets multicast packets through Android's Wi-Fi filter; null until first started. */
    private var multicastLock: WifiManager.MulticastLock? = null

    /** Answers `note_taker/microphone`; null until the engine is configured. */
    private var microphonePermission: MicrophonePermission? = null

    /** Copies what other apps share to the app; created with the activity. */
    private val incomingShares by lazy { IncomingShares(applicationContext) }

    /** Invite links opened from the camera (#122). */
    private val pairingLinks = PairingLinks()

    /** Reports charging to the core while the activity exists; created with it. */
    private val chargingReporter by lazy { ChargingReporter(applicationContext) }

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
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/share")
            .setMethodCallHandler(SharedFiles(this))
        val sharesChannel =
            MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/incoming_shares")
        sharesChannel.setMethodCallHandler(incomingShares)
        incomingShares.channel = sharesChannel
        val linksChannel =
            MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/pairing_links")
        linksChannel.setMethodCallHandler(pairingLinks)
        pairingLinks.channel = linksChannel
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/call_signals")
            .setMethodCallHandler(CallSignals(applicationContext))
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/background_core")
            .setMethodCallHandler { call, result -> handleBackgroundCore(call, result) }
        MethodChannel(flutterEngine.dartExecutor.binaryMessenger, "note_taker/npu")
            .setMethodCallHandler(NpuDownload())
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
                RecordingService.show(
                    this,
                    state,
                    call.argument<String>("title"),
                    call.argument<String>("silencedText"),
                )
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
        incomingShares.channel = null
        pairingLinks.channel = null
        super.cleanUpFlutterEngine(flutterEngine)
    }

    /**
     * Starts reporting charging, and copies a share that started the app. A restored activity
     * skips the share: its intent was imported before the process was killed.
     */
    override fun onCreate(savedInstanceState: Bundle?) {
        // Before the engine starts, so the core finds the NPU's libraries (#75).
        NpuDownload.prepareEnvironment(applicationContext)
        super.onCreate(savedInstanceState)
        chargingReporter.start()
        removeCallOfferChannel()
        if (savedInstanceState == null &&
            (pairingLinks.receive(intent) || incomingShares.receive(intent))
        ) {
            // Cleared so a later recreation of the activity does not import it again.
            setIntent(Intent(Intent.ACTION_MAIN))
        }
    }

    /** Stops reporting charging when the activity goes away. */
    override fun onDestroy() {
        chargingReporter.stop()
        super.onDestroy()
    }

    /** Copies a share that reached the running app (the activity is `singleTop`). */
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        if (!pairingLinks.receive(intent)) {
            incomingShares.receive(intent)
        }
    }

    /**
     * Deletes the "Calls" notification channel earlier versions made for the call offer (#111),
     * so it no longer shows in the app's notification settings. Side effects: None when absent.
     */
    private fun removeCallOfferChannel() {
        if (android.os.Build.VERSION.SDK_INT < android.os.Build.VERSION_CODES.O) {
            return
        }
        getSystemService(android.app.NotificationManager::class.java)
            ?.deleteNotificationChannel("call_offer")
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

    /**
     * Answers the background-pass channel (#87): `remember(configuration, keyDirectory)` stores
     * the core settings for `WaitingPassJob`; `isExempt` tells whether Koalino is exempt from
     * battery optimisation, which Power saving requires for the job to run; `requestExemption`
     * shows Android's dialog asking for it.
     */
    private fun handleBackgroundCore(call: MethodCall, result: MethodChannel.Result) {
        val powerManager = getSystemService(PowerManager::class.java)
        when (call.method) {
            "remember" -> {
                val configurationJson = call.argument<String>("configuration")
                val keyDirectory = call.argument<String>("keyDirectory")
                if (configurationJson == null || keyDirectory == null) {
                    result.error("invalid", "configuration and keyDirectory are required", null)
                } else {
                    BackgroundCoreConfiguration(applicationContext)
                        .remember(configurationJson, keyDirectory)
                    result.success(null)
                }
            }
            "isExempt" -> result.success(powerManager?.isIgnoringBatteryOptimizations(packageName))
            "requestExemption" -> {
                try {
                    startActivity(
                        Intent(Settings.ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS)
                            .setData(Uri.parse("package:$packageName"))
                    )
                    result.success(null)
                } catch (error: ActivityNotFoundException) {
                    result.error("unavailable", "no battery optimisation dialog", null)
                }
            }
            else -> result.notImplemented()
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

    /**
     * Releases the lock when the app is hidden; background sync is a separate task (#47).
     * Side effects: Schedules the charging job, which runs any waiting after-meeting pass once
     * the phone charges, even after the app is closed (#87); it ends at once when none waits.
     */
    override fun onStop() {
        multicastLock?.takeIf { it.isHeld }?.release()
        WaitingPassJob.schedule(applicationContext)
        super.onStop()
    }
}
