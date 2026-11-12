<!-- Documents headless Linux daemon background-recording integration without platform source code. -->

# Linux headless daemon and background recording

This directory reserves the server deployment boundary. The Rust daemon and FFI core own portable
recording state; Linux service management, device permissions, and suspend policy remain outside
the application layer.

## systemd deployment steps

1. Install the daemon and `libnote_taker_core.so` under a dedicated non-root service account with
   access to the intended audio device. Keep database, logs, and review-clip directories writable
   only by that account.
2. Create a `systemd` service that starts the headless daemon after sound-device availability and
   restarts it only on unexpected failure. Set a deliberate stop timeout so an explicit Stop FFI
   call can complete before service termination.
3. Use `systemd-inhibit --what=sleep:idle --why="Koalino recording"` (or an equivalent
   logind D-Bus inhibitor managed by the service supervisor) only while the FFI state is Recording.
   Release the inhibitor on Paused, Interrupted, Stopped, Error, or daemon shutdown.
4. Expose a local, access-controlled control surface that calls only
   `note_taker_start_recording`, `note_taker_pause_recording`, and
   `note_taker_stop_recording`. After every request, call `note_taker_get_current_state` and use
   its integer result to decide whether an inhibitor or status indicator is needed.
5. On ALSA/PipeWire/PulseAudio stream suspension or logind prepare-for-sleep, call
   `note_taker_handle_audio_interruption`. This moves the core to recoverable Interrupted instead
   of terminal Stop. After wake, reacquire the audio device and call Start only when policy allows.
6. Do not use a network-exposed unauthenticated endpoint for lifecycle control. Do not log raw
   audio, audio device buffers, E2EE material, or local file paths in service status output.

## State codes and concurrency

The FFI state query returns Idle `10`, Recording `11`, Paused `12`, Interrupted `13`, Stopped
`14`, or Error `15`; non-state errors remain below `10`. The Rust core uses a process-wide
`OnceLock<Mutex<...>>`, so systemd helpers and UI/control threads may call its no-argument C ABI
functions concurrently without sharing or retaining Rust pointers. A caller must still treat a
nonzero lifecycle status as rejected and re-query state instead of assuming it owns the recorder.
