<!-- Documents stable boundary contracts before external protocol and FFI implementations exist. -->

# API specifications

## Rust core ports

The initial public contracts are Rust traits, not a network protocol:

- `TranscriptionEngine` receives `AudioRecording` metadata and returns a `Transcript`. `ComputeDispatcher` invokes this trait only; model-specific and remote-proxy adapters are composed outside the application layer.
- `DeviceManifest` advertises a node ID, CPU core count, GPU VRAM, and supported `TranscriptionModel` values. It contains no raw audio, model weights, credentials, or hardware serial numbers. Every node sends its manifest to each paired device when it connects
  (`NTM1` followed by the manifest's JSON, over the same encrypted stream as `NTS1` sync
  envelopes; see `core/application/device_manifest_exchange.rs`). `GET /api/p2p/status` and
  `note_taker_p2p_status` show this device's `capabilities` and each connected paired device's
  (`cpu_cores`, `gpu_vram_megabytes`, `transcription_models` such as `whisper`/`openai`,
  `supports_vad`; `null` while a device is not connected).
- Each node with a pair-agreement secret also sends every connected paired device its X25519
  public key: `NTK1` followed by exactly 32 bytes, on the same encrypted stream
  (`core/application/pair_key_exchange.rs`). The pair key derived from it protects review clips
  sent between the two devices (Issue #50); see `core/application/crypto/README.md`.
- Unresolved unknown-speaker clusters travel as `NTC1` followed by a value sealed with the pair
  key (purpose `NTC1`, bound to sender and receiver). Inside: a big-endian `u32` JSON length, the
  JSON body (meeting and cluster IDs, creation time, total duration, representative embedding,
  member chunks with embeddings, clip metadata with each clip's `audio_length`), then the clips'
  raw audio in order. See `core/application/cluster_share_exchange.rs`.
- `ComputeDispatcher` accepts `LocalOnly`, `SpecificPeer`, or `Automatic` preferences. Automatic routing considers only model-compatible manifests and chooses the highest deterministic GPU-VRAM-then-CPU capacity score.
- `NoteRepository` creates a meeting with opaque CRDT bytes, persists and loads a `Transcript`, and atomically replaces or reads a serialized CRDT document. It returns only domain values and `StorageError`; it never exposes SQLite types.
- `P2pNetwork` permits only encrypted opaque-payload transport.
- `FileTransferNetwork` is a separate encrypted attachment transport port. Its private libp2p
  behaviour negotiates `/note-taker/file-transfer/1`, not the CRDT sync protocol. Its frames are a
  request (`NTFQ`, hash, offset) and a chunk (`NTFC`, hash, total size, offset, up to 64 KiB of data).
- `AttachmentLibrary` is the local side: it lists linked attachments, reads chunks of linked files
  only, and stores arriving files once verified. It exposes no SQLite or libp2p values.

- **Other networks (Issues #64, #65).** The status has `network`: `relay_address` (or `null`),
  `relay_state` (`not_configured`, `not_paired`, `connecting`, `connected`, `reserved`), `peers`, and
  `offers_relay`. The FFI command `set_network_addresses` (`relay`, `peers`) validates, stores, and
  applies them at once, and refuses with `invalid_relay_address` or `invalid_peer_address`, changing
  nothing. Saved values replace `NOTE_TAKER_P2P_RELAY` and `NOTE_TAKER_P2P_PEERS`.
- **Invites.** `POST /api/p2p/invites` and the FFI command `create_invite` open the pairing window
  and return `{"invite","expires_at_ms"}`. `POST /api/p2p/invites/join` and `join_invite` (`invite`)
  list the device as nearby and return `{"peer_id"}`. They refuse with `invite_malformed`,
  `invite_expired`, `invite_from_this_device`, `already_paired`, or `no_addresses` (HTTP `400`,
  `{"error_code"}`). The text is `note-taker-invite:1;<peer id>;<expires at ms>;<address>,<address>`,
  has no secret in it, and grants no trust: pairing still needs the numeric code.
- **Device names (Issue #121).** Each entry of `nearby_devices` may carry `device_name`, the name
  that device told over `/note-taker/device-name/1`. A device answers only over connections its
  peer gate admits, so only while its pairing window is open; until then the entry has
  `"device_name":null`. The name is a claim, as in a pairing request: pairing still needs the code.
  `PUT /api/p2p/device-name` (body `{"name"}`) and the FFI command `set_device_name` (`name`) save
  this device's name in `<db>.device-name` and announce it at once. They answer
  `{"device_name"}` (the trimmed name), or refuse with HTTP `400` / `invalid_device_name` (blank,
  over 64 characters, or a control character), `503` / `unavailable` (no database), or
  `storage_failed`. The saved name overrides `NOTE_TAKER_DEVICE_NAME`. The CLI has
  `devices rename <name>`. Devices paired before keep the name they learned while pairing.
- `PUT /api/p2p/transcription-route` (body `{"route": ...}`) and the FFI command
  `set_transcription_route` choose where chunks are transcribed: `local`, `auto`, or a paired
  device's peer ID. `transcription_route` in the P2P status shows the choice.

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
  NUL-terminated UTF-8 `tracing` filter directive (`error`, `warn`, `info`, `debug`, `trace`, or
  any valid directive). It returns `0`, `1` for a null, non-UTF-8, or invalid directive (nothing
  is stored), or `2` when the level cannot be stored or applied. It stores the level in
  `<db>.log-level`, so it survives restarts, and reloads the filter, starting file logging if it
  has not started. It does not need the runtime to be open (Issue #46).
- `char *note_taker_get_log_level(void)` returns the stored level, or `info` when none is stored.
  Free it with `note_taker_free_string`.
- `char *note_taker_export_logs(const char *output_path)` joins the newest seven daily log files,
  oldest first, into one JSON-lines file at `output_path` (absolute), or at
  `<export folder>/note_taker_logs_<YYYYMMDD-HHMMSS>.jsonl` when it is null or empty. It returns
  `{"ok":true,"file_path","size_bytes","file_count"}` or `{"ok":false,"error_code","error"}` with
  `invalid_argument`, `no_logs`, or `export_failed`; free it with `note_taker_free_string`. It
  does blocking file I/O, so call it from a background isolate.

The app starts file logging (`<data>/logs/app_log.<date>`) when the runtime first opens, at the
stored level.

Neither function accepts raw audio, voice embeddings, P2P credentials, Rust ownership pointers,
or serialized domain structures.

### Telemetry C ABI

Each function returns caller-freed UTF-8 JSON (release it with `note_taker_free_string`), or null
when the runtime is unavailable or the argument is invalid. The shapes match the HTTP telemetry
routes below.

- `char *note_taker_get_devices(void)` returns the device array (the `devices` value of
  `GET /api/devices`).
- `char *note_taker_get_telemetry_summary(void)` returns the `GET /api/telemetry/summary` object.
- `char *note_taker_get_device_telemetry(const char *device_id)` returns the
  `GET /api/telemetry/devices/{device_id}` object for the last 24 hours.

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

These functions call `application::cluster_review`, the same use case as the CLI and HTTP API.
They always return caller-freed JSON (release it with `note_taker_free_string`), and they return
`NULL` only if the response cannot be allocated. They run synchronously, so Dart should call them
from a background isolate.

- `char *note_taker_get_unknown_clusters(const char *meeting_id)` returns a JSON array in
  creation order:
  `[{"cluster_id":"uuid","meeting_id":"m1","total_duration_ms":2500,"chunk_count":2,"created_at":"RFC 3339","speaker_label":"Speaker 2","review_clip_path":"/abs/<clip>.wav"}]`.
  `speaker_label` is the voice's label in its meeting's transcript (#118), or `null` when no
  transcript segment carries it.
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

- `char *note_taker_get_voice_library(void)` returns every meeting's unresolved clusters and the
  saved people (Issue #63):
  `{"ok":true,"unknown_clusters":[{"cluster_id","meeting_id","meeting_title":"…|null","total_duration_ms","chunk_count","created_at","speaker_label":"Speaker 2|null","has_review_clip":true,"voice_group_id":"uuid","suggested_name":"…|null"}],"saved_people":[{"identity_id","display_name","voice_variant_count"}]}`,
  or `{"ok":false,"error_code":"list_failed|runtime_unavailable","error":"…"}`. It writes no audio.
  Voices that sound like one person, in any meeting, share `voice_group_id` (the ID of the group's
  first cluster) and are listed next to each other (Issue #82). A lone voice's group is its own ID.
  `suggested_name` is the name a call window or the Teams captions showed while the voice spoke
  (#113), or null; it is only offered, never assigned without the owner.
- `char *note_taker_assign_voice_group(const char *cluster_ids_json, const char *identity_name)`
  names several voices as one person (Issue #82). `cluster_ids_json` is a JSON array of UUID
  strings. It returns
  `{"ok":true,"identity_id","identity_name","identity_created","assigned_cluster_ids":["uuid"],"skipped_cluster_count":0,"variants_stored":2,"segments_updated":5}`.
  Each voice is assigned as by `note_taker_assign_cluster`, to one identity. Voices already named or
  gone are skipped and counted; when all are, it returns the first one's error. Errors use the codes
  of `note_taker_assign_cluster`; a malformed array, an empty one, or a blank name is
  `invalid_argument`.
- `char *note_taker_prepare_cluster_review_clip(const char *cluster_id)` writes that cluster's clip
  to `NOTE_TAKER_REVIEW_CLIP_DIR` when the owner asks to hear it, and returns
  `{"ok":true,"review_clip_path":"/abs/<clip>.wav"}`, or `{"ok":false,"error_code","error"}` with
  `invalid_argument`, `clip_not_found`, `clip_unavailable`, or `runtime_unavailable`. Assigning the
  cluster deletes the file.

### Attachment import C ABI

- `char *note_taker_process_attachment(const char *file_path)` accepts a non-empty local path and
  returns an owned UTF-8 JSON array or `NULL`. Every item includes one opaque `attachment_id`,
  `meeting_id`, `meeting_start_time`, `meeting_end_time`, and `confidence_score`. Ranking is
  performed by `AttachmentManager`; Flutter must call `note_taker_free_string` exactly once after
  copying a non-null response.
- `int32_t note_taker_confirm_attachment_link(const char *attachment_id, const char *meeting_id)`
  persists only an explicitly selected local metadata link. Both IDs are opaque strings returned by
  the prior suggestion response. It returns the standard `0` success / nonzero rejection statuses.
- `char *note_taker_attach_files(const char *meeting_id, const char *paths_json)` (Issue #54)
  attaches several pictures or documents to one meeting. `paths_json` is a non-empty JSON array
  of absolute paths. Each accepted file (JPEG, PNG, WebP, HEIC, PDF, TXT, Markdown, DOCX, XLSX,
  PPTX or ODT, up to 100 MiB) is copied privately to
  `<data_directory>/attachments/<meeting_id>/`. The source is left in place, so the app deletes
  its camera temporaries.
  - While the meeting is being recorded, attachments carry its current `meeting_time_ms`;
    otherwise it is null, which places them at the end of the meeting.
  - It returns `{"ok":true,"attached":[<attachment>],"failed":[{"path","error"}]}`. A file that
    fails does not stop the others.
  - A blank ID or a malformed or empty array returns `invalid_argument`; an unknown meeting
    returns `meeting_not_found`.

The ABI returns JSON strings rather than nested Rust structs. It neither accepts nor returns file
contents, EXIF data, or database-specific values.

### AI connections and summaries C ABI (#95)

Each returns owned JSON (free with `note_taker_free_string`), `{"ok":true,...}` or
`{"ok":false,"error_code","error"}`, with the error codes of the web routes (see "AI connections
and meeting summaries" below). Keys are write-only.

- `char *note_taker_llm_command(const char *request_json)` runs `settings`, `save_connection`,
  `delete_connection`, `update` or `test_connection`, tagged by `command`, with the same fields as
  the web routes.
- `char *note_taker_summarize_meeting(const char *meeting_id, const char *connection_id)` writes
  the meeting's summary with that connection, or the default one when `connection_id` is null or
  empty. It blocks while the model works (minutes with a local model), so call it off the UI
  thread; the runtime is locked only while the meeting is read and the summary stored.
- `char *note_taker_get_meeting_summary(const char *meeting_id)` returns `summary` (or null),
  `meeting_prompt` (or null) and `request` (or null), this meeting's request to a paired
  computer (#107): `{"state":"waiting"|"working"|"failed","requested_at", "by"?, "since"?,
  "error_code"?, "failed_at"?}`.
- `char *note_taker_request_paired_summary(const char *request_json)` (#107) asks a paired
  computer to write the meeting's summary, or cancels the request, from
  `{"meeting_id","cancel"?:bool}`. It writes the request (this device, the time, and the owner's
  own template when they wrote one) into the meeting document, which syncs to paired devices, and
  returns `{"request"}` (null once cancelled). A new request replaces an earlier, failed one.
  Nothing goes to an AI server from the asking device. Errors: `invalid_request`,
  `storage_failed`, `runtime_unavailable`.
- `char *note_taker_set_meeting_summary_prompt(const char *meeting_id, const char *prompt)` sets
  the meeting's own template, or clears it when `prompt` is null or blank.
- `char *note_taker_handoff_meeting(const char *request_json)` (#114) prepares a meeting for the
  owner's own AI app. The request is
  `{"meeting_id","purpose":"summary"|"chat","directory","inline"?}`, where `directory` is a folder
  the app owns for hand-offs. For a project's chat (#126), `project_id` replaces `meeting_id`
  (`purpose` must be `chat`): the Markdown holds every meeting of the project, oldest first, and
  the files are the project's own files and every meeting's attachments. `"all": true` instead
  hands over every note: every listed meeting (each naming its project) and every project's
  files; that Markdown always goes only as a file, too long to paste. With `"inline": true`
  (computers, #126), `message` also carries the Markdown, for the owner to paste into a website.
  Attachments always go as files: the Markdown only names them, never their text. The core empties
  the folder, then writes the Markdown and plain copies of the attachments (at most 20, 50 MiB
  each, 100 MiB in all).
  The answer is `message` (the summary prompt, or an introduction for a chat), `prompt` (the
  filled-in instructions; empty for a chat), `files` (`path`, `mime_type`; the Markdown first),
  `skipped` and `document_attached` (true when the message does not carry the Markdown). Errors: `invalid_request`, `nothing_to_summarize`, `storage_failed`. No AI connection
  is used; the app shares the files and empties the folder again.

### Meeting edits C ABI (#96)

- `char *note_taker_meeting_edit(const char *request_json)` runs one edit tagged by `command` and
  returns caller-freed JSON: `notes` (`meeting_id`) → `notes`; `set_notes` (`meeting_id`, `text`)
  → `notes`; `edit_summary` (`meeting_id`, `text`) → `summary`, marked `edited`; `edit_segment`
  (`meeting_id`, `start_time_ms`, `end_time_ms`, `text`) → `segment`, or null when blank text
  removed it; `keep_answer` (`meeting_id`, `text`, `source` such as `Claude app`, optional `prompt`)
  → `summary` and `previous` (the summary it replaced, or null), keeping an answer from the
  owner's own AI app as the summary, marked `edited` (#114); `restore_summary` (`meeting_id`,
  `summary` as `previous` gave it, or null to remove it) undoes that; `create_meeting` (optional
  `title`, optional `text` for the notes; no `meeting_id`) → `meeting_id` of a new meeting with
  no recording, filled by writing or adding files (#129). Error codes:
  `invalid_request`, `meeting_not_found`, `segment_not_found`, `empty`, `too_long`,
  `storage_failed`, `runtime_unavailable`.

### Search C ABI (#100)

- `char *note_taker_search(const char *request_json)` searches and returns caller-freed JSON.
  The request is `{"query","scope"?,"project_id"?}`, and the answer is as for
  `GET /api/search` below. Extra error code: `runtime_unavailable`.

### Chat C ABI (#97)

- `char *note_taker_chat(const char *request_json)` runs one command and returns caller-freed
  JSON: `{"command":"history"|"ask"|"clear","scope":"meeting"|"project"|"all","id"?,
  "question"?,"connection_id"?,"on_paired_computer"?}`. Answers and errors are as for
  `/api/chat` below, plus `runtime_unavailable`. `ask` blocks until the model answers.
- `"on_paired_computer": true` (#107, the phone) sends the question and the chat so far, sealed,
  to the connected paired computers one at a time. The first computer whose owner chose a
  connection under "Summaries for my other devices" answers from its own copy of the notes, and
  the phone keeps both messages in its own chat (the answer's `connection` reads like
  "Claude Code on Laptop"). Extra error codes: `no_paired_computer`, `paired_computer_timeout`
  (an accepted question gets 6 minutes), `paired_computer_left`, `rate_limited` (30 questions
  an hour per computer), `busy`, `p2p_unavailable`, or the computer's own code (such as
  `llm_unreachable`). A computer that does not offer chat declines at once; one that stays
  silent for 20 s (an older version) is skipped.

### Clear copies C ABI (#99)

- `char *note_taker_clear_copy(const char *attachment_id)` decrypts one attachment or project
  resource into the app's private `clear-copies` folder and returns caller-freed JSON:
  `{"ok":true,"path":"<copy>"}`, or `{"ok":false,"error_code","error"}` with `invalid_argument`,
  `not_found` (no such attachment, or its file has not arrived), `unreadable`, `storage_failed`
  or `runtime_unavailable`. Attachment files are sealed at rest; the `file_path` fields in other
  answers name the sealed file, which only the core can read. Copies are removed 30 minutes after
  they were made and when the app starts. Blocks while the file is decrypted.

### Notes folder C ABI (#98)

- `char *note_taker_notes_folder(const char *request_json)` runs one command and returns
  caller-freed JSON: `{"command":"get"}`, `{"command":"set","enabled","path"?}` (turning it on
  writes the folder at once) or `{"command":"sync_now"}`. The answer is the status of
  `GET /api/settings/notes-folder` below. Extra error code: `runtime_unavailable`.

### Call watch C ABI (#111)

- `char *note_taker_call_watch(const char *request_json)` returns caller-freed JSON. The requests:
  - `{"command":"poll","users"?}` takes one look at the apps capturing the microphone (it runs
    `pactl` on Linux and asks the audio sessions on Windows, so call it off the UI thread). A host
    that looks itself (Android) sends `"users":[{"process","name"}]` and the system is not asked;
    `android:voice-communication` is a call whose app is unknown, shown as `A call app`. It answers
    `{"ok":true,"available","in_call","app","event"}`, where `event` is `started` or `ended` on
    the look that noticed it, else null. `available` is false where the system cannot tell; a
    look whose tool failed this once answers `{"ok":false,"error_code":"probe_failed"}`.
  - `{"command":"get"}` and `{"command":"set","offer"?,"auto_stop"?,"read_names"?}` answer
    `{"ok":true,"offer","auto_stop","read_names","restart_apps"}`. `read_names` (on by default,
    #113) reads who call windows show as speaking during recordings. While it is on, these
    requests turn the desktop's accessibility flag on (Linux); `restart_apps` is true when it had
    to, because browsers and call apps already running must be restarted once.
- Error codes: `invalid_request`, `storage_failed`, `runtime_unavailable`, `probe_failed`. No
  open runtime is needed.
- `char *note_taker_call_toast(const char *request_json)` shows the offer as a Windows toast.
  `{"command":"offer","app"}` blocks until the owner answers, the offer is withdrawn or two
  minutes pass, and returns `{"ok":true,"answer"}` (`record`, `dismissed`, `open` for a click on
  the toast itself, or `unavailable`, always outside Windows); call it on a background thread.
  `{"command":"withdraw"}` hides the toast and returns `{"ok":true}`. The toast comes from the
  AppUserModelID `Koalino.NoteTaker`, registered under `HKCU\Software\Classes\AppUserModelId`.

### Assistant access C ABI (#112)

- `char *note_taker_assistant_access(const char *request_json)` runs `{"command":"get"}` or
  `{"command":"set","enabled":bool}` and returns caller-freed JSON:
  `{"ok":true,"enabled","database_path","server_path","config","claude_code","passphrase_needed"}`.
  `server_path` is the `note_taker_mcp` binary beside the app's executable, or null where it is
  not installed (Android). `config` is the `mcpServers` entry for Claude Desktop and similar
  clients and `claude_code` the `claude mcp add` line; both are null without the binary and never
  hold a key or passphrase. Error codes: `invalid_request`, `storage_failed`.

### Projects C ABI (Issue #85)

- `char *note_taker_project_command(const char *request_json)` runs one Projects command and
  returns owned JSON (free with `note_taker_free_string`): `{"ok":true,...}` or
  `{"ok":false,"error_code","error"}`. The request is tagged by `command`:

| `command` | Fields | Answer |
| --- | --- | --- |
| `list` | | `projects`: `[{"project_id","name","created_at","updated_at","meeting_count","resource_count","latest_meeting_at"}]` by name |
| `create` | `name` (1–200 characters, one line) | `project` |
| `rename` | `project_id`, `name` | `project` |
| `delete` | `project_id` | `deleted` (its meetings stay, outside any project; its resources and folder go) |
| `detail` | `project_id` | `project`, `meetings` (newest first, as in `note_taker_get_meetings`), `resources` (oldest first) |
| `move_meeting` | `meeting_id`, `project_id` (null moves it out) | `{}` |
| `meeting_projects` | | `meeting_projects`: `{meeting ID: project ID}` |
| `add_files` | `project_id`, `paths` | `added`, `failed` `[{"path","error"}]`, `skipped_attachments` |
| `add_note` | `project_id`, `title`, `text` | `resource` (a Markdown file) |
| `delete_resource` | `project_id`, `attachment_id` | `deleted: true` |
| `read_email` | `project_id`, `attachment_id` | `email`: `{"subject","from","to","cc","sent_at","body_text","body_truncated","attachments"}` |

- A resource is `{"attachment_id","file_name","mime_type","kind","added_at","size_bytes","file_path"}`;
  `kind` is `image`, `markdown`, `pdf`, `email`, `text` or `document` (opened outside the app), and
  `size_bytes` is null while the file is still on its way from a paired device.
- `add_files` accepts the attachment types above plus GIF and `.eml`; an e-mail is kept whole, and
  each of its attachments of an accepted type is added too (others count in `skipped_attachments`).
- Error codes: `invalid_request`, `invalid_name`, `project_not_found`, `meeting_not_found`,
  `resource_not_found`, `not_an_email`, `rejected`, `storage_failed`, `runtime_unavailable`.
- `note_taker_get_meetings` items carry `project_id` (null outside any project).

### Charging (after-meeting pass, #87)

- `int32_t note_taker_set_charging(int32_t charging)` reports whether the device is charging
  (nonzero) and returns `0`, or `3` when the runtime lock is poisoned. A phone runs the
  after-meeting pass only while charging. The state is remembered, so a report made before the
  app has opened the database applies once it does; the call never creates the runtime. With a
  runtime, charging while speech from an earlier run waits builds the meeting pipeline so the pass
  starts, which loads the model: call it off the main thread. Android calls it through JNI
  (`NativeCore.setCharging`). Desktop builds never need it.

### Background recording and lock-screen C ABI

- `int32_t note_taker_start_recording(void)`, `note_taker_pause_recording(void)`, and
  `note_taker_stop_recording(void)` may be called from Flutter, an OS foreground service, or a
  lock-screen action thread. They serialize through the process-wide Rust mutex and return `0` on
  accepted transitions, `1` for invalid input, `2` for state rejection, `3` for unavailable
  runtime state, and `4` (`FFI_PIPELINE_UNAVAILABLE`) when the meeting pipeline cannot be built
  (no transcription engine is configured; the Rust log names the variables to set). Builds with
  the `microphone` feature record the default microphone: a start or resume returns `7`
  (`FFI_MICROPHONE_UNAVAILABLE`) and records nothing when it cannot be opened (no input device,
  or access denied); pause, interruption, and stop turn it off (Issue #45).
- Since Issue #32 a start runs the meeting pipeline. The first start composes it (this may load a
  model) and later recordings reuse it. Start records into a new `meeting-<uuid>`, or resumes a
  paused or interrupted recording. A start after a stop begins a new recording. Stop flushes
  buffered audio and returns only after every chunk is transcribed and persisted, then deletes
  unprocessed raw audio. Call start and stop from a background isolate.
- `int32_t note_taker_start_recording_for_meeting(const char *meeting_id)` starts a recording into
  the given non-blank meeting, creating it when missing. A recording into an existing meeting
  continues after its last segment.
- `int32_t note_taker_start_recording_with_title(const char *meeting_id, const char *title)`
  (Issue #35) accepts null for either argument. A null `meeting_id` generates `meeting-<uuid>`,
  and a null `title` keeps the meeting's current title. Otherwise it behaves like
  `..._for_meeting` and stores the trimmed title on the meeting. Invalid UTF-8, a blank meeting
  ID, or a blank, over-200-character, or control-character title returns `1`. Both arguments are
  ignored when resuming a paused recording.
- `char *note_taker_get_meetings(void)` returns
  `[{"meeting_id","title","created_at","duration_ms","transcription_languages"}]`, and
  `char *note_taker_get_meeting(const char *meeting_id)` returns the same fields plus
  `transcript` (the flat transcript, null until stored) and `segments`
  (`[{"start_time_ms","end_time_ms","speaker","text","overlapping_speech","uncertain_speaker","uncertain_words"}]`
  in timeline order, where `speaker` is the voice-store name, `Speaker N` for an unnamed voice (numbered in order
  of first appearance in that meeting, #118), or `Unattributed`, as in exports; Issue #36; the confidence marks are described under
  "Transcript confidence"), and
  `attachments`
  (`[{"attachment_id","file_name","file_path","mime_type","meeting_time_ms"}]` in the order they
  were added; Issue #54). `meeting_time_ms` is where in the transcript the attachment belongs,
  or null for the end of the meeting.
  `duration_ms` is the end of the latest segment (0 without segments).
  `transcription_languages` lists the speech-to-text languages chosen while recording (`auto` or
  Whisper codes) in first-use order; it is empty for meetings recorded before they were kept. `title`
  is the stored title, or `Meeting YYYY-MM-DD HH:MM UTC` for an untitled meeting; `created_at`
  is RFC 3339 UTC. Both also carry `recorded_on`, `combined_recordings` and `combining`, as in the HTTP API
  (#88); the list leaves out recordings combined into another meeting. Errors use
  `{"ok":false,"error_code","error"}` with `invalid_argument`,
  `meeting_not_found`, `list_failed`, `load_failed`, or `runtime_unavailable`. Free results
  with `note_taker_free_string`.
- `int32_t note_taker_rename_meeting(const char *meeting_id, const char *new_title)` (Issue #38)
  replaces a meeting's title. It returns `0` on success; `1` for a null or non-UTF-8 pointer, a
  blank meeting ID, or an invalid title (the same rules as creation: trimmed, 1–200 characters,
  no control characters such as newlines); `2` for a storage failure; `3` for unavailable
  runtime state; and `5` (`FFI_MEETING_NOT_FOUND`) for an unknown meeting. A rejected call
  writes nothing. Read the new title back with `note_taker_get_meeting`.
- `int32_t note_taker_delete_meeting(const char *meeting_id)` deletes a meeting with its transcript,
  attachments and unknown voices, here and on paired devices, for good. Files linked from elsewhere
  stay. It returns `0`; `1` for a null, non-UTF-8 or unusable ID; `2` while it is being recorded or
  for a storage failure; `3` for unavailable runtime state; `5` for an unknown meeting. Recordings
  other devices made of the meeting, combined into it (#88), are deleted with it.
- `int32_t note_taker_split_meeting(const char *meeting_id)` (#88) lists the recordings combined
  into a meeting on their own again, for good; a combined recording's ID splits only that one. It
  returns `0`, also when there was nothing to split; `1` for a null, non-UTF-8 or blank ID; `2`
  for a storage failure; `3`; `5` for an unknown meeting.
- `int32_t note_taker_delete_unknown_voice(const char *cluster_id)` deletes an unknown voice's
  embeddings and review audio, here and on paired devices, for good. Its transcript stays. It
  returns `0`; `1` for a malformed UUID; `2` while its meeting is recording or for a storage failure;
  `3`; `5` for an unknown voice.
- `int32_t note_taker_delete_saved_person(const char *identity_id)` deletes a saved person's name,
  voice samples and external account links, here and on paired devices, for good. Transcripts stay
  and show an unknown speaker. It returns `0`, also for an unknown person; `1` for a malformed UUID;
  `2` for a storage failure; `3`.
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
- `char *note_taker_transcription_settings(void)` returns Settings' view as JSON (free it with
  `note_taker_free_string`): `selected_model`, `shipped_model`, `available_disk_bytes`, `models`
  and `download`. Transcription is always on the device: the OpenAI provider and its key were
  removed (#117).
- `int32_t note_taker_set_efficient_gpu(int32_t enabled)` turns efficient mode on or off for the next recording. It returns `0`, or
  `1` when turning it on without a supported GPU. `int32_t note_taker_retry_gpu(void)` lets the GPU be tried again after a crash
  stopped it, and returns `0` or `2`. `note_taker_transcription_settings` adds `suggestion` (`file_name`, `efficient`, `reason`)
  and `efficient` (`gpu_name`, `enabled`, `gpu_blocked`, `gpu_in_use`).
- `int32_t note_taker_set_npu_whisper(int32_t enabled)` turns Whisper on the NPU (#75) on or off from the next
  recording. It returns `0`, `1` when turning it on where it cannot run (a build without `npu-whisper`, a chip other than the
  SM8550, or files not all downloaded), or `2` when the choice can't be stored. `note_taker_transcription_settings` adds
  `npu` (`available`: the build has the engine; `supported`: and this phone's chip runs it; `directory`: where the Android
  host downloads the files; `installed`: every file is there; `enabled`: the owner's choice). The download itself is the
  Android host's `note_taker/npu` channel (`download(directory)`, `progress()`, `cancel()`, `remove(directory)`), see
  `ui/android/README.md`. When the NPU cannot open, the core transcribes with whisper.cpp.
- `int32_t note_taker_use_automatic_model(void)` chooses "Automatic" (#106) and returns `0` or `2`; choosing a model with
  `note_taker_set_whisper_model` leaves it. `selected_model` is then `null` (it no longer falls back to the shipped model), and
  `automatic` reports `enabled`, `file_name` (the model the next recording loads: the suggestion when it is on the device,
  else the shipped model), `efficient` and `waiting_for_download` (the suggested model while it isn't on the device). Nothing
  downloads by itself. `automatic.speed_check` (#109) is a sentence for Settings once that model was timed on a public clip
  on this hardware ("Checked on this device: 0.3× real time, so it keeps up with recordings.", or, when it is too slow,
  which smaller model keeps up), else `null`.
- `int32_t note_taker_set_after_meeting_pass(int32_t enabled)` turns the after-meeting pass (#87) on or off from the next
  recording, and returns `0` or `2` when the choice can't be stored. `note_taker_transcription_settings` reads it back as
  `after_meeting_pass` (a boolean, `true` unless turned off). A phone runs the pass only while charging.
- `int32_t note_taker_set_after_meeting_pass_model(const char *file_name)` chooses the model the pass re-transcribes
  with (#87): a catalog model on this device, or null or empty for the recording model. It returns `0`, `1` for a model
  that is unknown or not on this device, or `2` when the choice can't be stored. `note_taker_transcription_settings`
  reads it back as `after_meeting_pass_model` (the file name, or `null`).
- `int32_t note_taker_set_computer_sound(int32_t enabled)` turns recording what the computer plays (#108) on or off
  from the next start or resume, and returns `0` or `2` when the choice can't be stored. `note_taker_transcription_settings`
  reads it back as `computer_sound: {available, enabled}`: `available` is true on desktop builds with the
  `computer-sound` feature (Windows loopback, Linux PulseAudio/PipeWire monitor), and `enabled` is `true` unless turned
  off. The sound opens beside the microphone, is transcribed as its own track (segments marked as the computer's), and,
  once the microphone is heard following the speakers, the microphone's copies of those words are dropped.
- `char *note_taker_whisper_proposal(void)` returns a model proposed from this device's speed as
  JSON (`file_name`, `reason`, `measured`), the text `null` with nothing to propose, or a null pointer
  when the runtime is unavailable; free it with `note_taker_free_string`.
  `int32_t note_taker_dismiss_whisper_proposal(const char *file_name)` stops proposing a catalog model
  and returns `0`, or `1` for an unknown file (#68).
- `int32_t note_taker_set_whisper_model(const char *file_name)`,
  `note_taker_download_whisper_model(const char *file_name)`
  and `note_taker_remove_whisper_model(const char *file_name)` return `0` on success, `1` for an
  invalid argument, `2` on failure, `5` when another download runs, `6` when the file would not fit, and `7` for a model that can't be removed.
  Changes apply when the next recording starts.
- `void note_taker_cancel_whisper_download(void)` cancels the download and removes its partial file.

The core never accepts background-service handles, callbacks, or retained pointers over FFI.

## External meeting bridge ports

`MeetingBridge` provides standard-library receivers of provider-neutral `ExternalMediaChunk` and
`SlideCapture` values. An external participant always contains both a stable `external_id` for
mapping and a `display_name` for first-time local identity naming; optional email is metadata only.
`TeamsIdentityMapper` persists the external-to-local association through `ExternalIdentityRepository`.

A participant's stream is a track that may carry several people (a meeting room, #103), so the
session diarizes every chunk and credits it to the participant only while one voice speaks; see
`core/application/pipeline/README.md`. `IdentityMapper::keep_participant_voice(identity,
embedding) -> bool` is offered the voice once per meeting, after 30 s of clean speech of one voice
that the gallery does not match to someone else. Its default keeps nothing. `TeamsIdentityMapper`
stores it through `ExternalIdentityRepository::store_participant_voice` only when its voice-keeping
setting answers true. The daemon reads `keep_meeting_voices` from the transcription settings file
at every offer.

`GET /api/settings/meeting-voices` returns `{"keep":false}`: whether participants' voices are kept
(off by default). `PUT /api/settings/meeting-voices` with `{"keep":true}` saves it and returns it.
A write failure returns `500` `{"error_code":"save_failed","error"}`, and a daemon without a
settings file `503` `unavailable`. The CLI uses `meeting-voices [on|off]`.

`TeamsProcessor` uses only these ports plus `VoiceEmbeddingExtractor`, `VoiceVectorDB`,
`TranscriptionEngine`, and `AttachmentWorkflow`; no external service SDK type reaches the core.

## Headless HTTP and WebSocket API

The headless daemon binds locally by default. `GET /api/meetings` returns
`[{"meeting_id","title","created_at","project_id","transcription_languages"}]`. `GET
/api/meetings/{id}` adds the same `title`, `created_at` and `transcription_languages` (speech-to-text
language codes in first-use order, empty when none was recorded), a nullable transcript, and attachment metadata, but no CRDT or file bytes. `title`
is the stored title, or `Meeting YYYY-MM-DD HH:MM UTC` for an untitled meeting. `POST
/api/meetings` with `{"title":"..."}` (or `{}`) creates an empty meeting and returns `201` with
its summary. An invalid title returns `400` (Issue #35). `GET /api/voice-gallery` returns
`identities` (`identity_id`, `display_name`, `voice_variant_count`), `unknown_clusters` (the summary
below plus `meeting_title`, `has_review_clip`, `voice_group_id` and `suggested_name`, for every
meeting; voices that sound like one person share `voice_group_id` and are listed together, Issue #82;
`suggested_name` is the name a call window or the Teams captions showed while the voice spoke, or
`null`, #113), and `unassigned_clips`
(identifiers and timing only); voice vectors and raw audio are never serialized.
`GET /api/clusters/{id}/review-clip` returns `200` with the clip as `audio/wav` and
`Cache-Control: no-store`, or `404` when the cluster has none (Issue #63). `GET /api/telemetry` returns aggregate task count, duration, and latency.

### Transcript confidence (#90, #91)

`GET /api/meetings/{id}` lists `segments` like `note_taker_get_meeting`:
`[{"start_time_ms","end_time_ms","speaker","text","overlapping_speech","uncertain_speaker","uncertain_words"}]`
in time order (empty for a meeting imported as text).
- `overlapping_speech` (bool): diarization heard two or more voices at once for at least 0.5 s
  of the segment (or half of a shorter one), where words are often missed.
- `uncertain_speaker` (bool): the speaker is a guess. Either it was a weak voice match, or no
  voice was found within 2 s of the text.
- `uncertain_words` (int array): positions, from 0, among the text's whitespace-separated words,
  of the words Whisper gave a probability under 0.4. Remote and OpenAI transcription give no
  probabilities, so their words are never marked.

The JSON export carries the same three fields on each segment, and the Markdown and PDF exports
show them as `Name (?)`, `(overlapping speech)` and `word (?)`.

### One meeting recorded on several devices (#88)

A meeting recorded on the laptop and on a paired phone is two recordings that the app combines
into one meeting, under the earlier recording. `GET /api/meetings` lists only that meeting, and
it and `GET /api/meetings/{id}` add:

- `recorded_on`: `[{"device_id","name","this_device","connected"}]`, the meeting's own device
  first. `name` is this device's name, or the name a paired device gave when pairing, kept while
  it is away. It is null without P2P, or for a device that is not paired here or never sent its
  manifest. `connected` says whether that device is connected now; null for this device and
  without P2P.
- `combined_recordings`: the IDs of the recordings combined into it, which are not listed on
  their own.
- `combining`: `{"state","combine_on","waiting_for","kept_until"}`, how far combining the
  recordings got as this device sees it. `state` is `one_device`, `waiting` (speech from the
  `waiting_for` devices still has to reach `combine_on`), `ready_to_combine` (all speech is on
  this device; the pass runs when it is free), `sending` (this device still has to send its
  speech to `combine_on`), `combined`, or `not_combined` (speech from `waiting_for` did not
  arrive within three weeks). Devices are shaped as in `recorded_on`; `combine_on` is null for one
  device. `kept_until` (RFC 3339) is when the speech held here is deleted if not combined by
  then, null when none is held. Speech waits sealed for at most three weeks, or the window the
  owner chose (`/api/settings/retention`).

`POST /api/meetings/{id}/split` lists those recordings on their own again, for good, and returns
`200` `{"split":[<meeting IDs>]}`, empty when there was nothing to split. Passing a combined
recording's ID splits only that one. An unknown meeting gets `404`. Each split recording shows
its own live transcript again; the meeting keeps its transcript, which may already combine every
device's speech. `DELETE /api/meetings/{id}` also deletes the recordings combined into it.

### Projects (Issue #85)

`GET /api/meetings` items carry `project_id` (null outside any project). The routes below use the
same commands as the app (`core/bin/project_commands.rs`); resources never carry local paths.

| Route | Success |
| --- | --- |
| `GET /api/projects` | `200`, project summaries by name |
| `POST /api/projects` `{"name"}` | `201`, the summary |
| `GET /api/projects/{project_id}` | `200` `{"project","meetings","resources"}` |
| `PATCH /api/projects/{project_id}` `{"name"}` | `200`, the summary |
| `DELETE /api/projects/{project_id}` | `204` |
| `PUT /api/meetings/{meeting_id}/project` `{"project_id"}` (null moves it out) | `204` |
| `POST /api/projects/{project_id}/resources?name=<file name>`, the file as the body (up to 100 MiB) | `201` `{"added","skipped_attachments"}` |
| `POST /api/projects/{project_id}/notes` `{"title","text"}` | `201`, the resource |
| `DELETE /api/projects/{project_id}/resources/{attachment_id}` | `204` |
| `GET /api/projects/{project_id}/resources/{attachment_id}/file` | `200`, the bytes (`Content-Disposition: attachment` for the `document` kind, `inline` otherwise) |
| `GET /api/projects/{project_id}/resources/{attachment_id}/email` | `200` `{"subject","from","to","cc","sent_at","body_text","body_truncated","attachments"}` (HTML bodies as plain text) |

Failures answer `{"error_code","error"}`: `400` for a bad ID, name, title or file type, `404` for
an unknown project, meeting or resource (or a file not synced here yet), `503` when the daemon has
no project files, `500` for storage errors. Files are served with their stored type,
`X-Content-Type-Options: nosniff` and a `sandbox` Content Security Policy.

### Settings (Issue #46)

`GET /api/settings/log-level` returns `{"level":"info"}`, the daemon's level in effect.
`PUT /api/settings/log-level` with `{"level":"debug"}` applies a `tracing` directive until the
daemon restarts (`RUST_LOG` sets the start level; `cli log-level` changes it too, and the route
reports either) and returns `{"level"}`. An invalid directive returns `400`
`{"error_code":"invalid_level","error"}`. `GET /api/logs/export` returns the newest seven daily log
files, oldest first, as one `application/x-ndjson` attachment, or `404`
`{"error_code":"no_logs"}`. The entries were redacted when written. A process without log
control answers `503` `{"error_code":"unavailable"}`.

`GET /api/settings/glossary` returns `{"terms":["Marco Rossi","OKR"]}`, the names and terms that
steer transcription. `PUT /api/settings/glossary` with `{"terms":[...]}` trims them, keeps each
once regardless of case, saves them and returns the saved list; it applies from the next recording
and to the next after-meeting pass. A term over 60 characters or holding a control character, or a
list over 500 characters in all, returns `400` `{"error_code":"invalid_glossary","error"}`; a write
failure `500` `save_failed`; a daemon without a settings file `503` `unavailable`. The apps use
`note_taker_set_glossary(terms)` (newline-separated; returns 0 saved, 1 refused, 2 failed) and read
the saved terms as `glossary` in `note_taker_get_transcription_settings()`.

`GET /api/settings/after-meeting-pass` returns `{"enabled":true,"model":null,"models":[{"file_name","name"}]}`:
whether each meeting is transcribed again after Stop (#87), its model (`null` is the recording
model), and the catalog models on the daemon's machine. `PUT` with `{"enabled"?,"model"?}` (a
missing field is kept, `"model":null` returns to the recording model) saves and returns the same.
A model not on that machine returns `400` `invalid_model` and saves nothing; a write failure `500`
`save_failed`; a daemon without a settings file `503` `unavailable`.

`GET /api/settings/retention` returns `{"days":21,"min_days":1,"max_days":60}`: how many days a
recording's speech may wait to be combined with another device's recording of the same meeting
(#93). `PUT /api/settings/retention` with `{"days":7}` saves the window and returns it. The
after-meeting worker applies it at its next round, so speech older than a shorter window is deleted
then, and `kept_until` in each meeting's `combining` follows it. A window outside the bounds
returns `400` `{"error_code":"invalid_window","error"}`; a write failure `500` `save_failed`; a
daemon without a settings file `503` `unavailable`. The apps use
`note_taker_set_keep_speech_days(days)` (0 saved, 1 out of range, 2 failed) and read
`keep_speech` (`{"days","min_days","max_days"}`) in `note_taker_get_transcription_settings()`.

### AI connections and meeting summaries (#95)

`GET /api/settings/llm` returns `{"ok":true,"connections":[...],"default_connection","template",
"default_template","custom_template","auto_summarize","paired_summary_connection",
"placeholders"}`. Each connection is
`{"id","label","kind","base_url","model","context_tokens","plain_http","has_key","is_default",
"tool_installed"}`, where `kind` is `open_ai_compatible` (a server such as Ollama, llama.cpp,
LM Studio or vLLM, or an online service such as Gemini, DeepSeek, Qwen, Kimi, Mistral or
OpenRouter, #138) or (#105) `claude_code` or `codex`. The last two run the Claude
Code or Codex CLI installed and signed in on the computer running the core, so a Claude or ChatGPT
plan answers instead of API credit: they have no `base_url` and no key, their `model` may be
empty (the tool's default), and `tool_installed` says whether the tool was found. An
`open_ai_compatible` connection may have an API key (#138, partly reversing #117): it is
write-only, kept in the platform keychain (`note-taker-llm-<id>`; on Android a file sealed under
the database key), never returned or logged, and sent as `Authorization: Bearer` only over HTTPS
or to this computer. `has_key` says whether one is saved. The `anthropic` kind is refused (an old
saved Anthropic connection is dropped when read).

- `POST /api/settings/llm/connections` creates a connection from `{"label","kind","base_url",
  "model","context_tokens"}` plus optionally `"api_key"` (printable, no spaces, at most 512
  characters; absent keeps the saved one) or `"clear_key": true`; `PUT
  /api/settings/llm/connections/{id}` edits one. Both return `{"connection"}`; a key that can't be
  stored gives `keychain_failed`. Deleting a connection deletes its key. `base_url` must be HTTPS, or plain HTTP to a loopback or private LAN address.
- `DELETE /api/settings/llm/connections/{id}` removes it.
- `POST /api/settings/llm/connections/{id}/test` sends a fixed one-line request (no meeting
  content) and returns `{"reply"}`.
- `PUT /api/settings/llm` with any of `{"default_connection","template","auto_summarize"}` changes
  only the fields named; `"template":null` goes back to the built-in template. While
  `auto_summarize` is true (the default) and a default connection exists, the daemon and the app
  summarize each meeting recorded on their device in the last 7 days once its transcript is
  final and it has no summary (#101). Templates may use
  `{{title}}`, `{{date}}`, `{{time}}`, `{{duration}}`, `{{device}}`, `{{project}}`,
  `{{speakers}}`, `{{glossary}}` and `{{language}}`. `{{device}}` names the devices that
  recorded the meeting (by the names paired devices gave, joined with " + "; "another device"
  for one not paired here), not the one writing the summary; `{{duration}}` is rounded to the
  nearest minute and is at least `1 min` (#119).
- `PUT /api/settings/llm` with `{"paired_summary_connection": id or null}` (#107) chooses the
  connection that writes the summaries paired devices ask for, or stops answering them (the
  default). An unknown ID is `connection_not_found`, and removing the connection turns answering
  off. While it is set, the daemon and the desktop app answer each waiting request found in a
  meeting document, whatever `auto_summarize` says:
  - They check every 20 s for documents changed since the last check, and every meeting at
    start-up and every 5 minutes.
  - The request is marked `working` with this device and the time; another computer takes it
    over only after 30 minutes.
  - The summary is written with the asking device's template, or this computer's when it sent
    none (a meeting's own prompt still comes first). Its `connection_label` names this computer
    (`"Claude Code on Laptop"`). It is stored and the request removed in one write.
  - A failure marks the request `failed` with the error code and is not retried.
  - At most 10 requests are sent per hour; the rest wait.
- `GET /api/meetings/{id}/summary` returns `{"summary": {"text","connection_label","model",
  "created_at","prompt","edited","truncated"} or null, "meeting_prompt": text or null,
  "request": the meeting's request to a paired computer (#107, as in the C ABI) or null}`.
- `POST /api/meetings/{id}/summary` with `{"connection_id"?}` sends the meeting's transcript,
  notes and photos to that connection (the default when absent), stores the summary in the
  meeting document, which syncs to paired devices, and returns `{"summary","parts",
  "images_sent","images_skipped"}`. It can take minutes with a local model.
- `PUT /api/meetings/{id}/summary-prompt` with `{"prompt": text or null}` sets or clears the
  meeting's own template.

Errors are `{"ok":false,"error_code","error"}`: `400` `invalid_request`, `invalid_connection`,
`invalid_template`; `404` `connection_not_found`, `meeting_not_found`; `409` `no_connection`,
`nothing_to_summarize`; `422` `too_long`; `502` `llm_unreachable`,
`llm_unauthorized`, `llm_rate_limited`, `llm_rejected`, `llm_failed`, `llm_refused`,
`llm_invalid_response`, `llm_tool_missing`, `llm_tool_failed` (with `error_detail`, what Claude
Code or Codex said); `500` `save_failed`, `storage_failed`; `503`
`unavailable`. An `llm_rejected` error also carries `error_detail` when the AI server gave a
reason, for example `invalid_request_error: Your credit balance is too low…` (one line, at most
300 characters; shown to the owner, never logged) (#104). The chat routes do the same.

### Meeting edits (#96)

- `GET /api/meetings/{id}/notes` returns `{"notes": text or null}`, the owner's Markdown notes.
- `PUT /api/meetings/{id}/notes` with `{"text"}` (at most 200,000 characters) replaces them.
- `PUT /api/meetings/{id}/summary` with `{"text"}` replaces the summary's text, or writes one by
  hand, and returns `{"summary"}` marked `edited`. Blank text is `400` `empty`. Automatic
  summaries never replace a meeting's summary.
- `PUT /api/meetings/{id}/segments` with `{"start_time_ms","end_time_ms","text"}` corrects one
  segment (as listed in the meeting's `segments`), keeping its speaker and timing; blank text
  removes it. The answer is `{"segment": {"start_time_ms","end_time_ms","text"} or null}`. The
  first edit flags the transcript as edited: the after-meeting pass then leaves it alone and
  deletes the speech it kept.
- Failures: `400` `invalid_request`, `empty` or `too_long`; `404` `meeting_not_found` or
  `segment_not_found`; `500` `storage_failed`. Every edit syncs to paired devices.

### Search (#100)

- `GET /api/search?q=<words>&scope=<meetings|projects|project>&project=<id>` finds where words
  were said or written. `scope` defaults to `meetings`: every meeting's title, summary, notes and
  transcript. `projects` covers every project's name, files (names; the text of text, Markdown
  and e-mail files) and meetings; `project` covers one, and needs `project`. Case and accents
  are ignored, and every word must be in the same passage (a transcript segment, or one line).
- The answer is `{"ok":true,"hits":[...],"truncated":bool}`, at most 200 hits: meetings newest
  first, then each meeting's title, summary, notes and transcript in time order. Projects come
  by name, each with its name, its files as added, then its meetings. A hit is:
  - `place`: `meeting_title`, `summary`, `notes`, `transcript`, `transcript_text`,
    `project_name`, `resource_name` or `resource_text`;
  - `meeting`: `{"meeting_id","title","created_at"}` or null;
  - `project`: `{"project_id","name"}` or null;
  - `resource`: `{"attachment_id","file_name","mime_type","kind"}` or null;
  - `snippet`: `[{"text","match"}]`, the passage, cut around the first match when long;
  - for `transcript`, also `start_time_ms`, `end_time_ms` and `speaker`.
- Failures: `400` `invalid_request`, `empty`, `too_short` (under 2 characters) or `too_long`
  (over 200); `404` `project_not_found`; `500` `storage_failed`. Search reads sealed text in
  memory only; the query is never logged.

### Chat (#97)

- `GET /api/chat?scope=meeting|project|all&id=` answers `{"ok":true,"messages":[...]}`, oldest
  first. A message is `{"role":"user"|"assistant","text","created_at","included","omitted",
  "connection","sources"}`; `included` and `omitted` say what the model was given and what was left out
  for length. `sources` (#110) lists the places an answer cites, in order of first mention:
  `{"number","label","at_ms","meeting_id"}` for a meeting (with `at_ms` when it cites a moment
  in the transcript) or `{"number","label","at_ms","project_id","resource_id"}` for a file. The
  model is given the material numbered (`## [1] Meeting "…"`) and asked to cite `[n]` or
  `[n, hh:mm:ss]`; numbers outside the material and Markdown links are ignored. Each place is
  listed once, and a meeting cited at moments is not listed again without one (#120). Transcripts
  are given as `[hh:mm:ss] Speaker: text` lines, a new line at each change of speaker and at least
  every 30 s within a long turn, so the model has a time near every statement.
- `POST /api/chat` `{"scope","id"?,"question","connection_id"?}` asks the connection (the
  default one without `connection_id`) and answers `{"ok":true,"question","answer",
  "truncated","images_sent"}`; both messages are kept.
  - A meeting chat sends its summary, notes, transcript and photos.
  - A project chat sends its files' text and newest images, then its meetings, newest first.
  - The `all` chat sends a list of every meeting and project, then the meetings and files that
    best match the question.
  - What does not fit the connection's context becomes the summary only, then is left out.
- `DELETE /api/chat?scope=&id=` deletes the chat: `{"ok":true,"cleared":bool}`.
- Chats are kept on the daemon's device, sealed, and are not synced. Deleting a meeting or a
  project deletes its chat.
- Failures: `400` `invalid_request`, `empty`, `too_long` (over 4 000 characters); `404`
  `meeting_not_found`, `project_not_found`, `connection_not_found`; `409` `no_connection`,
  `nothing_to_chat`, `context_too_small`; `502` `llm_*`; `503` `unavailable`.
- `"on_paired_computer": true` in `POST /api/chat` (#107) has a paired computer answer, as the C
  ABI's `note_taker_chat` does (see the Chat C ABI). It needs no connection here and keeps both
  messages in this device's chat. Extra failures: `503` `p2p_unavailable`, `no_paired_computer`,
  `paired_computer_left`, `busy`; `504` `paired_computer_timeout`; `429` `rate_limited`; a
  computer's own code otherwise.

### Notes folder (#98)

- `GET /api/settings/notes-folder` answers the status: `{"ok":true,"enabled","path",
  "suggested_path","last_round_at","last_error","written","imported"}`. `path` is null until a
  folder is chosen; `suggested_path` is `~/NoteTaker` on the daemon's machine; `last_error` is the
  last round's error code, or null.
- `PUT /api/settings/notes-folder` `{"enabled", "path"?}` saves the setting. Without `path` the
  saved or suggested folder is used. Turning it on writes the folder at once.
- `POST /api/settings/notes-folder/sync` reads edits back and writes the folder now.
- The tree: `<Project>/` holds the project's files; meetings are in
  `<Project or "No project">/<YYYY-MM-DD HH-MM Title>/` (UTC) with `notes.md`, `summary.md` (when
  there is one), a read-only `transcript.md` and `attachments/`. Edits to notes, summaries and
  project files, and new files in a project folder or a meeting's `attachments/`, are taken in at
  the next round (every 30 seconds).
- Failures: `400` `invalid_request` or `invalid_folder` (not absolute, a filesystem root or the
  home folder itself); `500` `folder_failed` or `storage_failed`; `503` `unavailable`.

### Telemetry

All telemetry responses carry privacy-safe metrics only: task kinds, durations, latencies, and
power estimates, never task content. A storage failure returns `500`.

- `GET /api/telemetry` returns `{ "task_count": 3, "total_duration_ms": 4200, "average_latency_ms": 85 }`.
- `GET /api/telemetry/summary` returns
  `{ "total_transcription_time_ms": 4200, "average_latency_ms": 85, "total_tasks": 3, "power_estimate_mwh": 1.5 }`.
- `GET /api/devices` returns the devices seen in telemetry:
  `{ "devices": [{ "device_id": "device-…", "supports_vad": true, "supported_models": ["whisper-base"], "last_seen": 1761642300000 }] }`
  (`last_seen` in epoch milliseconds).
- `GET /api/telemetry/devices/{device_id}` returns that device's metrics from the last 24 hours:
  `{ "device_id": "device-…", "metrics": [{ "task_type": "transcription", "duration_ms": 1400, "latency_ms": 80, "power_mwh": 0.5, "recorded_at": 1761642300000 }] }`.
  `task_type` is `transcription`, `cloud_transcription`, `file_transfer`, or `diarization`. A
  device with no metrics in that window returns `404`.

`GET /ws` upgrades to a WebSocket and sends tagged JSON text events: `meeting_started` with an
opaque `meeting_id`, `sync_status_changed` with a safe status string, or `new_unassigned_clip`
with opaque meeting/clip IDs. The router allows browser CORS only from `http://localhost:3000`. Every route, `/ws` included, also refuses with `403` any
request whose `Origin` is present and is not `http://localhost:3000` (CORS does not cover WebSocket
handshakes), or whose `Host` is a DNS name other than `localhost` (DNS rebinding); IP-literal hosts
and requests without these headers (CLI, scripts) are served. See `core/server/request_guard.rs`.

`POST /api/voice/assign` accepts JSON `{ "clip_id": "uuid", "identity_id": "uuid" }` and
returns `204` after the existing voice identity service writes a derived signature variant. Invalid
identifiers return `400`, a missing clip or an identity the voice store has never saved returns
`404`, and no raw audio or embedding is returned.

### Meeting rename

`DELETE /api/meetings/{id}` deletes a meeting with its transcript, attachments and unknown voices,
`DELETE /api/clusters/{uuid}` an unknown voice (its transcript stays), and
`DELETE /api/identities/{uuid}` a saved person's name and voice samples. They are irreversible and
reach paired devices. Responses: `204` when done; `404` for an unknown meeting or voice (a person
always gets `204`); `409` while the meeting is being recorded; `400` for a malformed UUID; `500` for a
storage failure. Errors are `{ "error": "..." }`.

`PATCH /api/meetings/{id}` (Issue #38) accepts `{ "title": "New Title" }`. `title` is required
and other fields are rejected. The title is trimmed and must be 1–200 characters without control
characters (such as newlines), the same rules as `POST /api/meetings`. Responses:

- `200` with the meeting summary: `{ "meeting_id": "meeting-1", "title": "New Title", "created_at": "2026-10-27T09:05:00+00:00" }`.
- `400` with `{ "error": "invalid title: meeting title must not be blank" }` (or the length or
  control-character reason). Nothing is written.
- `404` with `{ "error": "meeting meeting-1 does not exist" }`.
- A malformed body (missing `title`, non-string `title`, unknown field) gets axum's `4xx`.
- `500` with `{ "error": "unable to rename the meeting" }` for storage failures.

CORS allows `PATCH` from `http://localhost:3000`. The rename is written through the meeting's
Automerge document (Issue #39), so it replicates to peers on the next sync.

The web client's `renameMeeting` (`web/lib/api.ts`, Issue #42) throws `MeetingRenameError` with
`kind` `invalid_title` (400), `not_found` (404), `server` (5xx), `unexpected` (any other status,
such as axum's 422 for a malformed body), or `network` (no response). The dashboard shows its own
messages and never displays the server's `error` text.

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
[{ "cluster_id": "uuid", "meeting_id": "meeting-1", "total_duration_ms": 2500, "chunk_count": 2, "created_at": "2026-10-27T09:15:00+00:00", "speaker_label": "Speaker 2" }]
```

`speaker_label` is the label the meeting's transcript gives that voice ("Speaker N", numbered in
order of first appearance, #118), or `null` when no segment carries it; the voice gallery uses it.

A meeting without clusters, including an unknown meeting, returns `[]`. Representative and member
embeddings are never serialized.

`POST /api/clusters/{cluster_id}/assign` accepts `{ "identity_name": "Grace Hopper" }`. A known
identity whose name matches after trimming, ignoring case, is reused. Any other name gets a new
identity UUID. On success it returns `200`:

```json
{ "cluster_id": "uuid", "identity_id": "uuid", "identity_name": "Grace Hopper", "identity_created": true, "variants_stored": 2, "segments_updated": 1 }
```

The call stores one voice variant per member embedding, relinks the cluster's transcript segments,
resolves the cluster, and deletes its review clips. The resolution is recorded in the meeting's
synced document, so every paired device holding a copy of the cluster resolves it and deletes its
copies of the clips (Issue #50). When two devices name the same cluster at once, all of them
settle on the same identity. A non-UUID `cluster_id` or a blank name
returns `400`, an unknown cluster `404`, an already-resolved cluster `409`, and storage failures
`500`. The identity and its name are persisted in the voice store, so they appear in
`GET /api/voice-gallery` and survive a daemon restart.

`POST /api/voice-groups/assign` accepts `{ "cluster_ids": ["uuid", …], "identity_name": "Marco" }`
and names each voice as one person, as above (Issue #82). It returns `200` with
`{ "identity_id", "identity_name", "identity_created", "assigned_cluster_ids": ["uuid"], "skipped_cluster_count": 0, "variants_stored", "segments_updated" }`.
Voices already named or gone are skipped and counted. A blank name or an empty list returns `400`, a
malformed body `422`, and when every voice was skipped, `404` or `409` as for the first one.

## MCP server for the owner's AI assistant (#112)

`note_taker_mcp` (desktop bundles, beside the app's executable) speaks the Model Context Protocol
over stdio: JSON-RPC 2.0, one message per line. It needs `NOTE_TAKER_DATABASE_PATH` set to an
existing database (it never creates one) and unlocks it as the app does (the platform keychain,
or the `NOTE_TAKER_ENCRYPTION_*` variables). Standard output carries only protocol messages; logs
go to `NOTE_TAKER_LOG_DIR`, by default `logs` beside the database, and name tools, never content.

- Methods: `initialize` (protocol versions `2025-06-18`, `2025-03-26`, `2024-11-05`; another
  request gets the newest), `ping`, `tools/list`, `tools/call`. Notifications get no reply.
  Errors: `-32700` (not JSON), `-32600`, `-32601` (unknown method), `-32602` (unknown tool).
- Tools, all `readOnlyHint`; results are one text block of JSON:
  - `list_meetings {project_id?, limit? = 50 (1..500)}` → `{"meetings":[{"meeting_id","title",
    "created_at","project_id"}],"total"}`, newest first, without meetings folded into others.
  - `read_meeting {meeting_id, include_transcript? = true}` → `{"meeting_id","title",
    "started_at","summary","notes","transcript"}`, the transcript as `[hh:mm:ss] Speaker: text`.
  - `search {query, scope?, project_id?}` → the hits of `GET /api/search`.
  - `list_projects {}` → `{"projects":[...]}` as `GET /api/projects`.
  - `read_project {project_id}` → the project, its meetings and its files, without file paths.
  - `read_project_file {project_id, attachment_id}` → `{"attachment_id","mime_type","text"}` for
    text, Markdown, CSV, e-mail, PDF, Word, PowerPoint, Excel and LibreOffice files (#128).
- A failed tool answers `isError: true` with `<code>: <message>` (`meeting_not_found`,
  `project_not_found`, `resource_not_found`, `no_text`, `invalid_request`, `storage_failed`, or a
  search code). While access is off (`<db>.assistant-access.json`, read at every call), every
  tool answers `isError: true` with how to turn it on, and reads nothing.

## Linux CLI contract

`note-taker` accepts `record start [--input <file.wav|->] [--meeting-id <id>] [--title <title>]
[--language <auto|code>]` (without `--input`, the default microphone in builds with the
`microphone` feature; without `--language`, `NOTE_TAKER_WHISPER_LANGUAGE`, else `auto`; the
language used is stored in the meeting's `transcription_languages`),
`record stop` (asks the running `record start` on the same database to finish and waits until
its audio is transcribed), `record status`, `meetings list` (ID, title,
date, duration), `meetings view <meeting-id>` (title first, then the transcription languages),
`meetings delete <meeting-id> --yes`, `voice delete-cluster <cluster-id> --yes` and
`voice delete-person <identity-id> --yes` delete for good, here and on paired devices. Without `--yes`
they delete nothing and say so. A meeting being recorded is refused.

`meetings rename <meeting-id> <new-title>` (prints `Renamed meeting <id>: "<old>" -> "<new>"`),
`voice list`, `voice assign <clip-uuid> <identity-uuid>`, `voice list-clusters <meeting-id>`,
`voice assign-cluster <cluster-uuid> <identity-name>`, `devices list` (peer ID, name, pairing
time), `devices revoke <peer-id|name>` (removes the device from the allowlist in the shared
database; a running daemon on it drops and disconnects the device within a second), `telemetry`,
`log-level <directive>` (writes `<database>.log-level`; a running daemon on the same database
applies it within 2 seconds without restarting), the global `--log-level <level>` flag (this
command's own logging; by default the log file records `info` and stdout only `error`),
`meetings export <meeting-id> --format <markdown|pdf|json> [--output <path>]`, and
`meetings export-all --format <format> [--date-range <start>:<end>]` (see `core/cli/README.md`). It calls only the
portable `AudioCapture`, repository, and `VoiceIdentityManager` boundaries. Voice assignment
receives UUIDs from clap, locates the review clip through `NoteRepository`, and stores the derived
variant through the manager; raw audio and embedding values never appear in terminal output.
