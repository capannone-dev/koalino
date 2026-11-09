// Tells the Rust core whether the phone is charging (#87). A phone runs the after-meeting
// transcript pass only while charging, and the core cannot see the battery itself. The sticky
// ACTION_BATTERY_CHANGED broadcast gives the state at once and again whenever the battery changes,
// so a pass waiting from an earlier run starts at the next report after the core has opened the
// database. Calls go to a background thread, because starting a pass loads the Whisper model.
package com.example.note_taker_ui

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.BatteryManager
import android.os.Build
import java.util.concurrent.Executors

/** Watches the battery while the activity exists and reports charging to `NativeCore`. */
class ChargingReporter(private val context: Context) {
    /** Runs core calls off the main thread, one at a time and in order. */
    private val executor = Executors.newSingleThreadExecutor()

    /** The last state sent; null before the first report. */
    private var lastReported: Boolean? = null

    private val receiver = object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
            report(intent)
        }
    }

    /**
     * Starts watching. Inputs: None. Outputs: None.
     * Side effects: Registers a receiver and reports the current state.
     */
    fun start() {
        val filter = IntentFilter(Intent.ACTION_BATTERY_CHANGED)
        val current = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            context.registerReceiver(receiver, filter, Context.RECEIVER_NOT_EXPORTED)
        } else {
            context.registerReceiver(receiver, filter)
        }
        current?.let(::report)
    }

    /** Stops watching. Side effects: Unregisters the receiver and ends the worker thread. */
    fun stop() {
        try {
            context.unregisterReceiver(receiver)
        } catch (error: IllegalArgumentException) {
            // Never registered; nothing to undo.
        }
        executor.shutdown()
    }

    /**
     * Sends the state when it changes, and every report while charging, so a waiting pass starts
     * once the core is ready. Inputs: A battery broadcast. Side effects: Calls the core.
     */
    private fun report(intent: Intent) {
        val charging = intent.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0
        if (charging == lastReported && !charging) {
            return
        }
        lastReported = charging
        if (executor.isShutdown) {
            return
        }
        executor.execute {
            try {
                NativeCore.setCharging(charging)
            } catch (error: LinkageError) {
                // An older core without the entry point: the pass waits, as before.
            }
        }
    }
}
