<!-- Documents stable boundary contracts before external protocol and FFI implementations exist. -->

# API specifications

## Rust core ports

The initial public contracts are Rust traits, not a network protocol:

- `TranscriptionEngine` receives `AudioRecording` metadata and returns a `Transcript`. `ComputeDispatcher` invokes this trait only; model-specific and remote-proxy adapters are composed outside the application layer.
- `DeviceManifest` advertises a node ID, CPU core count, GPU VRAM, and supported `TranscriptionModel` values. It contains no raw audio, model weights, credentials, or hardware serial numbers. A future encrypted P2P adapter broadcasts these manifests.
- `ComputeDispatcher` accepts `LocalOnly`, `SpecificPeer`, or `Automatic` preferences. Automatic routing considers only model-compatible manifests and chooses the highest deterministic GPU-VRAM-then-CPU capacity score.
- `NoteRepository` creates a meeting with opaque CRDT bytes, persists and loads a `Transcript`, and atomically replaces or reads a serialized CRDT document. It returns only domain values and `StorageError`; it never exposes SQLite types.
- `P2pNetwork` permits only encrypted opaque-payload transport.
- `VoiceVectorDB` stores `VoiceEmbedding` variants, searches a derived query vector, and removes
  all variants for a UUID identity. It accepts no raw audio. Similarity calculations remain
  adapter-owned; application policy receives only scored candidates.
- `AudioCapture` controls a capture lifecycle and offers receiver-based subscriptions to portable PCM `AudioChunk` and `AudioState` events. It exposes neither OS audio stream types nor P2P transport types, allowing local-device and future streaming adapters to be substituted.
- `MetricsRepository` persists only completed AI task metadata and current non-sensitive device-load snapshots. It saves `TaskMetrics`, filters records by device ID, and returns aggregate task count, total duration, and average latency. It accepts no audio, transcript text, model inputs, outputs, embeddings, credentials, or network payloads.
- `MetadataExtractor` accepts an adapter-managed file path and returns only a UTC timestamp. `AttachmentManager` uses it with `NoteRepository::get_meetings_overlapping` to produce midpoint-ranked `MeetingSuggestion` values, without importing file-format APIs. `AttachmentRepository` persists attachment metadata and optional `MeetingId` links without accepting file bytes.

Concrete libp2p, SQLite, Automerge, model, REST, and FFI protocols will be specified alongside their adapters. No central routing protocol is defined.

## Flutter C ABI

The Rust `cdylib` exports only primitive C-compatible functions. Flutter owns a Dart
`RecordingBridge` interface and its `RustRecordingBridge` adapter; presentation and controller
code receive that interface and never access Rust structs.

- `int32_t note_taker_start_recording(void)` returns `0` when the Rust-owned local recorder accepts
  its start transition. `1` is reserved for invalid arguments, `2` for an operation rejected by
  the Rust core, and `3` when the bridge runtime cannot be acquired.
- `int32_t note_taker_set_log_level(const char *filter_directive)` accepts a non-null,
  NUL-terminated UTF-8 `tracing` filter directive and returns the same status codes. It installs
  or reloads Rust's JSON stdout and rolling-file logging without passing logging structs to Dart.

Neither function accepts raw audio, voice embeddings, P2P credentials, Rust ownership pointers,
or serialized domain structures.

### Voice Gallery C ABI

- `char *note_taker_get_unassigned_clips(const char *meeting_id)` returns an owned UTF-8 JSON
  array or `NULL` on invalid input or failure. Each object contains `clip_id`, `start_time_ms`,
  `end_time_ms`, `local_file_path`, and optionally `weak_match` with `identity_id` and
  `confidence_score`. `local_file_path` is absolute; raw audio bytes and embeddings never cross
  FFI. Call `note_taker_free_string` exactly once on a non-null result.
- `int32_t note_taker_confirm_weak_match(const char *clip_id, const char *identity_id)` validates
  the loaded local review clip and delegates derived-vector storage to `VoiceIdentityManager`.
- `int32_t note_taker_assign_new_identity(const char *clip_id, const char *display_name)` validates
  its strings and delegates first-variant creation to `VoiceIdentityManager` using the review
  clip's retained derived embedding.

The Rust bridge materializes review audio in `NOTE_TAKER_REVIEW_CLIP_DIR` (default
`voice_review_clips`) and the Flutter audio package opens the returned file path. The ABI never
streams audio bytes to Dart.

### Attachment import C ABI

- `char *note_taker_process_attachment(const char *file_path)` accepts a non-empty local path and
  returns an owned UTF-8 JSON array or `NULL`. Every item includes one opaque `attachment_id`,
  `meeting_id`, `meeting_start_time`, `meeting_end_time`, and `confidence_score`. Ranking is
  performed by `AttachmentManager`; Flutter must call `note_taker_free_string` exactly once after
  copying a non-null response.
- `int32_t note_taker_confirm_attachment_link(const char *attachment_id, const char *meeting_id)`
  persists only an explicitly selected local metadata link. Both IDs are opaque strings returned by
  the prior suggestion response. It returns the standard `0` success / nonzero rejection statuses.

The ABI returns JSON strings rather than nested Rust structs. It neither accepts nor returns file
contents, EXIF data, or database-specific values.

### Background recording and lock-screen C ABI

- `int32_t note_taker_start_recording(void)`, `note_taker_pause_recording(void)`, and
  `note_taker_stop_recording(void)` may be called from Flutter, an OS foreground service, or a
  lock-screen action thread. They serialize through the process-wide Rust mutex and return `0` on
  accepted transitions, `1` for invalid input, `2` for state rejection, and `3` for unavailable
  runtime state.
- `int32_t note_taker_handle_audio_interruption(void)` records a platform audio suspension. It
  moves an active stream to recoverable Interrupted and returns `0`; callers resume via Start.
- `int32_t note_taker_get_current_state(void)` returns only state codes: Idle `10`, Recording
  `11`, Paused `12`, Interrupted `13`, Stopped `14`, Error `15`. Runtime error codes remain below
  `10`, so lock-screen hosts can distinguish a state response from an unavailable runtime.

The core never accepts background-service handles, callbacks, or retained pointers over FFI.

## External meeting bridge ports

`MeetingBridge` provides standard-library receivers of provider-neutral `ExternalMediaChunk` and
`SlideCapture` values. An external participant always contains both a stable `external_id` for
mapping and a `display_name` for first-time local identity naming; optional email is metadata only.
`TeamsIdentityMapper` persists the external-to-local association through `ExternalIdentityRepository`.
`TeamsProcessor` uses only these ports plus `VoiceEmbeddingExtractor`, `VoiceVectorDB`,
`TranscriptionEngine`, and `AttachmentWorkflow`; no external service SDK type reaches the core.
