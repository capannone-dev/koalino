//! Tries the microphone's ways in, in order, until one opens (#152).
//!
//! On Linux the microphone first opens through ALSA's `default` device. Some systems have no ALSA
//! `default` routed to the sound server (PipeWire without `pipewire-alsa`), and there `default`
//! falls back to the first sound card's raw device, often playback-only. The adapter then tries
//! the sound server directly. This module holds the part that needs no audio library: run each
//! attempt in turn, keep the first that opens, and word one reason when none does.

/// What to try next when no microphone opened on Linux, appended to the combined reason.
pub const MICROPHONE_FIX_HINT: &str = "if PipeWire or PulseAudio runs, installing its ALSA plugin \
     (pipewire-alsa, or pulseaudio-alsa) gives ALSA a default microphone";

/// One way to open the microphone: a short label for logs, and the function that tries it.
///
/// The function returns the open stream, or a privacy-safe reason (it names the failing step,
/// never audio).
pub type MicrophoneAttempt<'attempt, Stream> = (
    &'static str,
    Box<dyn FnOnce() -> Result<Stream, String> + 'attempt>,
);

/// Runs `attempts` in order and returns the first stream that opens, with its label.
///
/// Inputs: The attempts, most preferred first; and a hint to end the reason with when none opens
/// (empty for none).
/// Outputs: The stream and the label of the attempt that opened it; or one reason that names
/// every attempt with its own reason, then the hint.
/// Side effects: Those of the attempts that run; later attempts do not run once one succeeds.
/// Each failure before a success is logged as a warning, so a fallback is visible in the logs.
pub fn open_first<Stream>(
    attempts: Vec<MicrophoneAttempt<'_, Stream>>,
    hint: &str,
) -> Result<(Stream, &'static str), String> {
    let mut failures: Vec<String> = Vec::new();
    for (label, attempt) in attempts {
        match attempt() {
            Ok(stream) => {
                if !failures.is_empty() {
                    tracing::warn!(
                        opened_with = label,
                        earlier_failures = %failures.join("; "),
                        "The microphone opened through a fallback"
                    );
                }
                return Ok((stream, label));
            }
            Err(reason) => failures.push(format!("{label}: {reason}")),
        }
    }
    let mut reason = String::from("no microphone could be opened");
    if !failures.is_empty() {
        reason.push_str(" (");
        reason.push_str(&failures.join("; "));
        reason.push(')');
    }
    if !hint.is_empty() {
        reason.push_str("; ");
        reason.push_str(hint);
    }
    Err(reason)
}
