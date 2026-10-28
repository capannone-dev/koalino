<!-- Documents the local adapter that implements the portable audio-capture port. -->

# Local audio adapter

`local_recorder.rs` implements the `AudioCapture` port with a deterministic, in-memory stream seam. It owns a terminal state machine: `Idle → Recording → Paused → Recording`, with `Recording`, `Paused`, or `Interrupted` allowed to transition to `Stopped`. An OS interruption moves `Recording → Interrupted`, returns `AudioError::Interrupted`, emits a structured warning, and can resume through Start. Invalid commands return `AudioError::InvalidState`; a stopped recorder is not reusable.

Subscribers receive adapter-neutral signed 16-bit PCM `AudioChunk` values only while the recorder is in `Recording`. The adapter never logs raw samples. Platform integrations may feed their OS capture callbacks into this adapter boundary without leaking platform types into ports or application code. A later P2P streaming adapter can implement `AudioCapture` directly and expose the same chunk/state subscriptions.

## Live recording support (Issue #32)

| File | Purpose |
| --- | --- |
| `vad_chunker.rs` | Cuts PCM into chunks of at most 15 s (Issues #51, #52). With the `vad` feature, a pause ends a chunk once it holds 3 s. A forced cut lands at the end of the quietest 30 ms frame of the last 3 s, and the audio after it starts the next chunk, so no sample is transcribed twice. `drain_ready_chunks` returns chunks oldest first, `finish` flushes the rest as `StreamEnded`, and `time_based` selects time cuts at runtime. |
| `chunk_audio_store.rs` | `ChunkAudioStore`: in-memory, delete-on-read PCM per chunk `RecordingId`. It implements the engines' audio sources (`AudioFileSource` as WAV and, with `local-ai`, `RecordingPcmSource`). Samples are wiped on read and on `clear`, so raw audio never outlives its transcription. `discard` and `discard_with_prefix` remove one producer's chunks only (local recordings use `<uuid>-chunk-<n>`, and the Teams bridge pump uses `teams-<uuid>`, discarding each chunk after processing). |
| `pcm_input.rs` | `PcmInput`: reads 16 kHz mono 16-bit PCM in 100 ms blocks from a WAV file (validated header, bounded allocation) or a raw stream such as stdin. |
| `pcm_resampler.rs` | `PcmResampler`: converts interleaved device audio (f32, i16, i32, u16; any rate and channel count) to 16 kHz mono PCM16. It averages channels, low-passes above 16 kHz (Blackman windowed sinc, 7 kHz cutoff), and interpolates linearly from an exact integer phase, so the output does not depend on how the device splits buffers (Issue #45). |
| `cpal_microphone.rs` | `CpalMicrophone` (`microphone` feature): the `MicrophoneSource` for the default input device through `cpal` (ALSA, so PipeWire and PulseAudio too, on Linux; WASAPI on Windows; AAudio on Android). Each stream lives on its own `microphone` thread because `cpal` streams are not `Send`; the audio callback resamples and sends blocks. Linux builds need `libasound2-dev`. |
| `live_microphone.rs` | `LiveMicrophone`: an open microphone stream plus its block queue. `drain` and `wait_for_frames` take queued audio; `close` turns the microphone off and returns the tail, so nothing captured before a stop is lost. |
| `fake_microphone.rs` | Test-only `FakeMicrophone`: counts opens and closes, can fail to open, and lets a test `speak` blocks through the open stream. |

`core/bin/recording_controller.rs` composes these with `LocalRecorder` into the live-recording
driver used by the FFI and CLI. With the `microphone` feature the FFI runtime and the CLI record
the default microphone themselves (Issue #45); a host can still push PCM through
`note_taker_push_audio_pcm` in builds without it, and the CLI reads `--input` when given.

The Flutter UI, rather than this Rust adapter, is responsible for permission prompts (Android's
`RECORD_AUDIO` through `MicrophonePermission.kt`), foreground/background services, wake-locks, and lock-screen controls. Those UI facilities invoke the port lifecycle methods; they do not alter this state machine. OS integrations report suspended audio through `handle_interruption` and use Start to resume after wake.
