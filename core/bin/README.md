<!-- This directory contains executable composition roots that wire ports to adapters at runtime. -->

# Binary entry points

`cli.rs` is the command-line composition root and `daemon.rs` is the headless service composition root. They own process-level configuration such as logging. `ffi.rs` owns the C-compatible Flutter boundary: it exposes primitive status-code functions only, keeping Rust domain structs and adapter implementations out of Dart.

Attachment import uses a caller-freed JSON string for ranked metadata only. `ffi.rs` composes
`AttachmentManager` with the local metadata and SQLite adapters, then receives opaque IDs back for
the user-confirmed persistence action; it never accepts attachment bytes from Flutter.

For background services and lock-screen controls, `ffi.rs` owns its recorder in a process-wide
`OnceLock<Mutex<...>>`; no FFI caller receives or retains a Rust pointer. Start, Pause, Stop,
interruption, and state-query commands take only this mutex and operate on the portable
`AudioCapture` state machine. State query codes are deliberately `10` through `15`, leaving the
documented nonzero status codes distinguishable from lifecycle states.

## Live recordings (Issue #32)

- `pipeline_composition.rs` is the only module that names concrete AI adapters. `PipelineConfig`
  reads `NOTE_TAKER_TRANSCRIPTION_PROVIDER` (`auto`, `local`, `openai`), `NOTE_TAKER_WHISPER_MODEL`,
  `OPENAI_API_KEY` (redacted from `Debug`), `NOTE_TAKER_OPENAI_WHISPER_MODEL`,
  `NOTE_TAKER_OPENAI_LANGUAGE`, `NOTE_TAKER_ENABLE_DIARIZATION`, and `NOTE_TAKER_VAD_ENABLED`.
  `auto` prefers a local whisper.cpp model and uses OpenAI only when a key is set (logged, since
  chunk audio then leaves the device). `spawn_recording_controller` builds the pipeline on the
  worker thread, with its own SQLite connections sharing the verified cipher and the encrypted
  voice library as matcher. No diarization adapter exists yet, so segments are `Unattributed`.
- `recording_controller.rs` (`RecordingController`) drives one recording: PCM → `LocalRecorder`
  (lifecycle; audio is dropped while paused) → `VadChunker` → `ChunkAudioStore` plus a
  `VadAudioChunk` with meeting-relative offsets → `RecordingManager`. A recording into an existing
  meeting continues after its last segment. Interruption flushes buffered audio. Stop waits for
  every chunk and wipes raw audio that no engine consumed.
- `ffi.rs` composes the controller on the first `note_taker_start_recording` and reuses it for
  every later recording. A failure returns `FFI_PIPELINE_UNAVAILABLE` (`4`) and is retried on the
  next start. `note_taker_start_recording_for_meeting`, `note_taker_push_audio_pcm`, and
  `note_taker_get_active_meeting_id` complete the ABI (see `API_SPECS.md`). Stop blocks while the
  last chunks are transcribed, so Flutter must call it from a background isolate.
- `cli_recording.rs` runs `record start --input <wav|->` in the foreground until the input ends, on
  Ctrl+C, or on `record stop`. Processes coordinate through `<db>.recording-active` (PID and
  meeting) and `<db>.recording-stop`. A lock left by a dead process is replaced.
- `daemon.rs` composes the pipeline at start-up on a blocking thread and logs its availability (a
  missing engine is not fatal). On SIGINT or SIGTERM it shuts down gracefully, letting the worker
  drain.

`storage_composition.rs` (`open_encrypted_stores`) is shared by all three roots. It opens the
`encryption_meta` record, unlocks and verifies the embedding key through `KeyManager`
(`NOTE_TAKER_ENCRYPTION_PASSPHRASE`, else the key file in `NOTE_TAKER_ENCRYPTION_KEY_DIR` or
`~/.note_taker`), and only then opens `SqliteNoteRepository` and `SqliteVoiceVectorDB` with one
shared cipher. A wrong key stops start-up with a readable error before any embedding is read.

`daemon.rs` composes the headless Axum server from the encrypted `SqliteNoteRepository` and
`SqliteVoiceVectorDB`, passing them only through `ServerAppState`. HTTP route handlers never use SQLite
directly; they consume repository ports. The daemon defaults to loopback binding and must sit
behind an authenticated reverse proxy before any non-loopback deployment.

`cli.rs` is the interactive terminal composition root. It parses the `core/cli` clap model, opens
the same `NOTE_TAKER_DATABASE_PATH` as the daemon, and passes portable adapters to the thin CLI
service. It fixes structured log filtering at `error` so a user's table/detail output is not mixed
with operational logs.

`ffi.rs` also exports `note_taker_export_meeting(meeting_id, format, options_json)`, which runs the
shared `ExportManager` and returns caller-freed JSON describing the written file or a structured
error (see `API_SPECS.md`). The CLI binary composes an `ExportWriter` from
`adapters::export::default_exporters()` and `NOTE_TAKER_EXPORT_DIR`.

`ffi.rs` exports two Voice Gallery commands that call `application::cluster_review`, the same use
case as the CLI and HTTP API. `note_taker_get_unknown_clusters(meeting_id)` returns unresolved
unknown-speaker clusters with optional local review-clip paths. `note_taker_assign_cluster(
cluster_id, identity_name)` assigns a cluster to a named identity and deletes its review audio.
Both return caller-freed JSON; see `API_SPECS.md`.
