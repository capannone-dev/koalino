// Serves the app lock on the `note_taker/app_lock` channel: the stored setting, whether the
// device has a screen lock, and the system BiometricPrompt (strong biometrics or the screen
// lock's PIN, pattern, or password). The framework prompt is used, so no AndroidX library is
// needed. Locking itself is decided in Dart (`AppLockController`); the Rust core keeps running.
package io.github.capannone_dev.koalino

import android.app.Activity
import android.app.KeyguardManager
import android.content.Context
import android.hardware.biometrics.BiometricManager
import android.hardware.biometrics.BiometricPrompt
import android.os.Build
import android.os.CancellationSignal
import io.flutter.plugin.common.MethodCall
import io.flutter.plugin.common.MethodChannel

/** Handles the app-lock channel for `activity`, whose window shows the prompt. */
class AppLock(private val activity: Activity) : MethodChannel.MethodCallHandler {
    private val preferences =
        activity.getSharedPreferences(PREFERENCES_NAME, Context.MODE_PRIVATE)

    /** Whether the owner turned the app lock on. */
    val isEnabled: Boolean
        get() = preferences.getBoolean(ENABLED_KEY, false)

    /**
     * Answers `status`, `authenticate`, and `setEnabled(enabled)`.
     *
     * Side effects: `authenticate` shows the system prompt; `setEnabled` writes the setting and
     * updates the recent-apps preview. Every call answers exactly once.
     */
    override fun onMethodCall(call: MethodCall, result: MethodChannel.Result) {
        when (call.method) {
            "status" -> result.success(
                mapOf(
                    "supported" to isSupported(),
                    "deviceSecure" to isDeviceSecure(),
                    "enabled" to isEnabled,
                )
            )
            "authenticate" -> authenticate(result)
            "setEnabled" -> {
                val enabled = call.argument<Boolean>("enabled")
                if (enabled == null) {
                    result.error("invalid_argument", "enabled is required", null)
                } else {
                    preferences.edit().putBoolean(ENABLED_KEY, enabled).apply()
                    applyRecentsPreview()
                    result.success(null)
                }
            }
            else -> result.notImplemented()
        }
    }

    /**
     * Hides the app's content from the recent-apps screen while the lock is on, as banking apps
     * do (Android 13 and later). Screenshots stay allowed.
     *
     * Side effects: Changes the activity's recents snapshot setting.
     */
    fun applyRecentsPreview() {
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
            activity.setRecentsScreenshotEnabled(!isEnabled)
        }
    }

    /** The prompt with a screen-lock fallback needs Android 10 (API 29). */
    private fun isSupported(): Boolean = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q

    /** Whether a PIN, pattern, or password is set (biometrics require one). */
    private fun isDeviceSecure(): Boolean =
        (activity.getSystemService(Context.KEYGUARD_SERVICE) as KeyguardManager).isDeviceSecure

    /**
     * Shows the prompt and answers true when the owner authenticated, false when they cancelled
     * or it gave up; `unsupported` when it cannot be shown.
     *
     * Side effects: Shows the system prompt over the activity; the PIN screen is a separate
     * activity, so the app is briefly hidden.
     */
    private fun authenticate(result: MethodChannel.Result) {
        if (!isSupported() || !isDeviceSecure()) {
            result.error("unsupported", "no screen lock to authenticate with", null)
            return
        }
        val builder = BiometricPrompt.Builder(activity)
            .setTitle("Unlock Koalino")
            .setSubtitle("Use your fingerprint, face, or screen lock")
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            builder.setAllowedAuthenticators(
                BiometricManager.Authenticators.BIOMETRIC_STRONG or
                    BiometricManager.Authenticators.DEVICE_CREDENTIAL
            )
        } else {
            @Suppress("DEPRECATION")
            builder.setDeviceCredentialAllowed(true)
        }
        var answered = false
        val answerOnce = { authenticated: Boolean ->
            if (!answered) {
                answered = true
                result.success(authenticated)
            }
        }
        try {
            builder.build().authenticate(
                CancellationSignal(),
                activity.mainExecutor,
                object : BiometricPrompt.AuthenticationCallback() {
                    override fun onAuthenticationSucceeded(
                        authenticationResult: BiometricPrompt.AuthenticationResult
                    ) = answerOnce(true)

                    // Cancelled, locked out, or timed out; one failed attempt is not final.
                    override fun onAuthenticationError(
                        errorCode: Int,
                        errorString: CharSequence,
                    ) = answerOnce(false)
                },
            )
        } catch (error: RuntimeException) {
            // For example a missing permission; the prompt never showed.
            if (!answered) {
                answered = true
                result.error("unsupported", error.message, null)
            }
        }
    }

    private companion object {
        const val PREFERENCES_NAME = "note_taker_app_lock"
        const val ENABLED_KEY = "enabled"
    }
}
