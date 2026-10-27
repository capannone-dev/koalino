<!-- Documents stable boundary contracts before external protocol and FFI implementations exist. -->

# API specifications

## Rust core ports

The initial public contracts are Rust traits, not a network protocol:

- `TranscriptionEngine` receives `AudioRecording` metadata and returns a `Transcript`. `ComputeDispatcher` invokes this trait only; model-specific and remote-proxy adapters are composed outside the application layer.
- `DeviceManifest` advertises a node ID, CPU core count, GPU VRAM, and supported `TranscriptionModel` values. It contains no raw audio, model weights, credentials, or hardware serial numbers. A future encrypted P2P adapter broadcasts these manifests.
- `ComputeDispatcher` accepts `LocalOnly`, `SpecificPeer`, or `Automatic` preferences. Automatic routing considers only model-compatible manifests and chooses the highest deterministic GPU-VRAM-then-CPU capacity score.
- `NoteRepository` creates a meeting with opaque CRDT bytes, persists and loads a `Transcript`, and atomically replaces or reads a serialized CRDT document. It returns only domain values and `StorageError`; it never exposes SQLite types.
- `P2pNetwork` permits only encrypted opaque-payload transport.
- `FileTransferNetwork` is a separate encrypted attachment transport port. Its private libp2p
  behaviour negotiates `/note-taker/file-transfer/1`, not the CRDT sync protocol. `FileStorage`
  stores verified content by SHA-256 and exposes no SQLite or libp2p values.

## Attachment file-transfer framing

An attachment transfer frame has the explicit header `{ hash, total_size, chunk_size, offset,
is_final }` plus at most 64 KiB of bytes. The receiver accepts only contiguous ascending offsets,
requires `received_bytes == total_size` on the final frame, enforces a 100 MiB object limit, and
verifies SHA-256 before atomic CAS commit. Zero-byte objects use one final frame; no empty-payload
sentinel is used. Availability announcements contain `FileReference` metadata only.
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

### Meeting export C ABI

`char *note_taker_export_meeting(const char *meeting_id, const char *format, const char *options_json)`
exports one meeting through the shared `ExportManager` and always returns caller-freed JSON
(release it with `note_taker_free_string`):

- Success: `{"ok":true,"file_path":"/abs/path.pdf","format":"pdf","size_bytes":1234,"metadata":{"schema_version":"1.0","meeting_count":"1"}}`.
- Failure: `{"ok":false,"error_code":"…","error":"…"}`, where `error_code` is
  `invalid_argument`, `unsupported_format`, `meeting_not_found`, `export_failed`, or
  `runtime_unavailable`.

`format` accepts `markdown`/`md`, `pdf`, or `json`. `options_json` may be null, empty, or an
object with optional `include_attachments`, `include_metadata` (both default `true`), and
`output_path`. Unknown keys are rejected. The call writes files synchronously, so Dart must invoke
it from a background isolate.

### Voice Gallery C ABI

- `char *note_taker_get_unassigned_clips(const char *meeting_id)` returns an owned UTF-8 JSON
  array or `NULL` on invalid input or failure. Each object contains `clip_id`, `start_time_ms`,
  `end_time_ms`, `local_file_path`, and optionally `weak_match` with `identity_id` and
  `confidence_score`. `local_file_path` is absolute; raw audio bytes and embeddings never cross
  FFI. Call `note_taker_free_string` exactly once on a non-null result.
- `int32_t note_taker_confirm_weak_match(const char *clip_id, const char *identity_id)` validates
  the loaded local review clip and delegates derived-vector storage to `VoiceIdentityManager`. It
  is rejected when the identity was never saved in the voice store.
- `int32_t note_taker_assign_new_identity(const char *clip_id, const char *display_name)` validates
  its strings, saves a new identity with the trimmed `display_name` in the voice store, and
  delegates first-variant creation to `VoiceIdentityManager` using the review clip's retained
  derived embedding.

The Rust bridge materializes review audio in `NOTE_TAKER_REVIEW_CLIP_DIR` (default
`voice_review_clips`) and the Flutter audio package opens the returned file path. The ABI never
streams audio bytes to Dart.

### Unknown-speaker cluster C ABI

Both functions call `application::cluster_review`, the same use case as the CLI and HTTP API.
They always return caller-freed JSON (release it with `note_taker_free_string`), and they return
`NULL` only if the response cannot be allocated. They run synchronously, so Dart should call them
from a background isolate.

- `char *note_taker_get_unknown_clusters(const char *meeting_id)` returns a JSON array in
  creation order:
  `[{"cluster_id":"uuid","meeting_id":"m1","total_duration_ms":2500,"chunk_count":2,"created_at":"RFC 3339","review_clip_path":"/abs/<clip>.wav"}]`.
  `review_clip_path` is present only while the cluster still has a review clip. The clip is written
  to `NOTE_TAKER_REVIEW_CLIP_DIR` for local playback. A meeting without clusters, including an
  unknown meeting, returns `[]`. On failure the function returns
  `{"ok":false,"error_code":"invalid_argument|list_failed|runtime_unavailable","error":"…"}`.
  Embeddings never cross FFI.
- `char *note_taker_assign_cluster(const char *cluster_id, const char *identity_name)` returns
  `{"ok":true,"cluster_id":"uuid","identity_id":"uuid","identity_name":"Grace Hopper","identity_created":true,"variants_stored":2,"segments_updated":1}`.
  A known identity with the same name (trimmed, ignoring case) is reused. Any other name gets a
  new identity UUID. After success the cluster's materialized review files are deleted. On failure
  it returns `{"ok":false,"error_code","error"}`, where `error_code` is `invalid_argument` (bad UUID
  or blank name), `cluster_not_found`, `cluster_already_resolved`, `assignment_failed`, or
  `runtime_unavailable`.

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
  accepted transitions, `1` for invalid input, `2` for state rejection, `3` for unavailable
  runtime state, and `4` (`FFI_PIPELINE_UNAVAILABLE`) when the meeting pipeline cannot be built
  (no transcription engine is configured; the Rust log names the variables to set).
- Since Issue #32 a start runs the meeting pipeline. The first start composes it (this may load a
  model) and later recordings reuse it. Start records into a new `meeting-<uuid>`, or resumes a
  paused or interrupted recording. A start after a stop begins a new recording. Stop flushes
  buffered audio and returns only after every chunk is transcribed and persisted, then deletes
  unprocessed raw audio. Call start and stop from a background isolate.
- `int32_t note_taker_start_recording_for_meeting(const char *meeting_id)` starts a recording into
  the given non-blank meeting, creating it when missing. A recording into an existing meeting
  continues after its last segment.
- `int32_t note_taker_push_audio_pcm(const int16_t *samples, size_t sample_count)` copies 16 kHz
  mono signed 16-bit PCM from the platform capture into the active recording. It returns `0`
  (audio is discarded while paused), `1` for a null pointer with a nonzero count, or `2` when no
  recording is active. It never waits for transcription, and the samples are never logged.
- `char *note_taker_get_active_meeting_id(void)` returns the recording's meeting ID (free it with
  `note_taker_free_string`), or null when no recording is active.
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

## Headless HTTP and WebSocket API

The headless daemon binds locally by default and accepts no mutation endpoints in this first
version. `GET /api/meetings` returns meeting-ID metadata; `GET /api/meetings/{id}` adds a nullable
transcript and attachment metadata but no CRDT or file bytes. `GET /api/voice-gallery` returns
speaker labels and unresolved review-clip identifiers/timing only; voice vectors and raw audio are
never serialized. `GET /api/telemetry` returns aggregate task count, duration, and latency.

`GET /ws` upgrades to a WebSocket and sends tagged JSON text events: `meeting_started` with an
opaque `meeting_id`, `sync_status_changed` with a safe status string, or `new_unassigned_clip`
with opaque meeting/clip IDs. The router allows browser CORS only from `http://localhost:3000`.

`POST /api/voice/assign` accepts JSON `{ "clip_id": "uuid", "identity_id": "uuid" }` and
returns `204` after the existing voice identity service writes a derived signature variant. Invalid
identifiers return `400`, a missing clip or an identity the voice store has never saved returns
`404`, and no raw audio or embedding is returned.

### Meeting export

`POST /api/meetings/{id}/export` accepts JSON
`{ "format": "markdown|md|pdf|json", "options": { "include_attachments": true, "include_metadata": true } }`
(`options` and each flag are optional and default to `true`). It returns `200` with the rendered
file as the body, `Content-Type` `text/markdown; charset=utf-8`, `application/pdf`, or
`application/json`, and `Content-Disposition: attachment; filename="<generated name>"`, which CORS
exposes to the web client. Unknown or reserved formats (`srt`) return `400`, an unknown meeting
returns `404`, and other failures return `500`. Every error body is `{ "error": "message" }`, and
500 responses carry no internal details. The handler extracts data under the repository lock and
renders on a blocking thread into a per-request temp directory that is deleted before responding.
A Markdown download contains only the `.md` file, so its attachment links are not bundled.

### Unknown-speaker clusters

Both routes call `application::cluster_review`, the same use case as the CLI and FFI. Error
bodies are `{ "error": "message" }`.

`GET /api/meetings/{id}/unknown-clusters` returns `200` with the meeting's unresolved clusters in
creation order:

```json
[{ "cluster_id": "uuid", "meeting_id": "meeting-1", "total_duration_ms": 2500, "chunk_count": 2, "created_at": "2026-10-27T09:15:00+00:00" }]
```

A meeting without clusters, including an unknown meeting, returns `[]`. Representative and member
embeddings are never serialized.

`POST /api/clusters/{cluster_id}/assign` accepts `{ "identity_name": "Grace Hopper" }`. A known
identity whose name matches after trimming, ignoring case, is reused. Any other name gets a new
identity UUID. On success it returns `200`:

```json
{ "cluster_id": "uuid", "identity_id": "uuid", "identity_name": "Grace Hopper", "identity_created": true, "variants_stored": 2, "segments_updated": 1 }
```

The call stores one voice variant per member embedding, relinks the cluster's transcript segments,
resolves the cluster, and deletes its review clips. A non-UUID `cluster_id` or a blank name
returns `400`, an unknown cluster `404`, an already-resolved cluster `409`, and storage failures
`500`. The identity and its name are persisted in the voice store, so they appear in
`GET /api/voice-gallery` and survive a daemon restart.

## Linux CLI contract

`note-taker` accepts `record start|stop|status`, `meetings list`, `meetings view <meeting-id>`,
`voice list`, `voice assign <clip-uuid> <identity-uuid>`, `voice list-clusters <meeting-id>`,
`voice assign-cluster <cluster-uuid> <identity-name>`, `telemetry`,
`meetings export <meeting-id> --format <markdown|pdf|json> [--output <path>]`, and
`meetings export-all --format <format> [--date-range <start>:<end>]` (see `core/cli/README.md`). It calls only the
portable `AudioCapture`, repository, and `VoiceIdentityManager` boundaries. Voice assignment
receives UUIDs from clap, locates the review clip through `NoteRepository`, and stores the derived
variant through the manager; raw audio and embedding values never appear in terminal output.
