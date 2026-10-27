<!-- Documents the local adapter that implements the portable audio-capture port. -->

# Local audio adapter

`local_recorder.rs` implements the `AudioCapture` port with a deterministic, in-memory stream seam. It owns a terminal state machine: `Idle → Recording → Paused → Recording`, with `Recording`, `Paused`, or `Interrupted` allowed to transition to `Stopped`. An OS interruption moves `Recording → Interrupted`, returns `AudioError::Interrupted`, emits a structured warning, and can resume through Start. Invalid commands return `AudioError::InvalidState`; a stopped recorder is not reusable.

Subscribers receive adapter-neutral signed 16-bit PCM `AudioChunk` values only while the recorder is in `Recording`. The adapter never logs raw samples. Platform integrations may feed their OS capture callbacks into this adapter boundary without leaking platform types into ports or application code. A later P2P streaming adapter can implement `AudioCapture` directly and expose the same chunk/state subscriptions.

## Live recording support (Issue #32)

| File | Purpose |
| --- | --- |
| `vad_chunker.rs` | Cuts PCM at speech pauses (`vad` feature) or every 30 s. `drain_ready_chunks` returns chunks oldest first, `finish` flushes the rest as `StreamEnded`, `time_based` selects time cuts at runtime, and `FinalizedAudioChunk::overlap_sample_count` reports the 1 s soft-max lookback. |
| `chunk_audio_store.rs` | `ChunkAudioStore`: in-memory, delete-on-read PCM per chunk `RecordingId`. It implements the engines' audio sources (`AudioFileSource` as WAV and, with `local-ai`, `RecordingPcmSource`). Samples are wiped on read and on `clear`, so raw audio never outlives its transcription. |
| `pcm_input.rs` | `PcmInput`: reads 16 kHz mono 16-bit PCM in 100 ms blocks from a WAV file (validated header, bounded allocation) or a raw stream such as stdin. |

`core/bin/recording_controller.rs` composes these with `LocalRecorder` into the live-recording
driver used by the FFI and CLI. There is no microphone adapter yet: Flutter (or another platform
host) pushes captured PCM through `note_taker_push_audio_pcm`, and the CLI reads `--input`.

The Flutter UI, rather than this Rust adapter, is responsible for permission prompts, foreground/background services, wake-locks, and lock-screen controls. Those UI facilities invoke the port lifecycle methods; they do not alter this state machine. OS integrations report suspended audio through `handle_interruption` and use Start to resume after wake.
