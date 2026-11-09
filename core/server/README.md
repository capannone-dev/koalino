<!-- Documents the headless HTTP and WebSocket adapter boundary. -->

# Headless server adapter

This directory will host the Axum adapter for the Linux headless mode. Its router receives
application-facing services and repository ports through dependency injection; handlers must never
access SQLite or other storage-specific APIs directly. Tests use in-memory SQLite plus Axum's
in-process request service so CI does not require a fixed network port.

## Runtime configuration

`daemon` binds `NOTE_TAKER_BIND_ADDRESS` (default `127.0.0.1:8080`) and opens
`NOTE_TAKER_DATABASE_PATH` (default `note_taker.sqlite3`). Keep the default loopback bind unless a
separately authenticated reverse proxy is configured. CORS permits only `http://localhost:3000`. Every route, `/ws` included, also refuses with `403` any
request whose `Origin` is present and is not `http://localhost:3000` (CORS does not cover WebSocket
handshakes), or whose `Host` is a DNS name other than `localhost` (DNS rebinding); IP-literal hosts
and requests without these headers (CLI, scripts) are served. See `core/server/request_guard.rs`.

## HTTP contract

- `GET /api/meetings` returns `[{"meeting_id","title","created_at"}]` metadata only. `title`
  is the stored title, or `Meeting YYYY-MM-DD HH:MM UTC` for untitled meetings; `created_at` is
  RFC 3339 UTC.
- `POST /api/meetings` accepts `{"title":"..."}` or `{}` and returns `201` with the new
  meeting's summary (generated `meeting-<uuid>` ID). An invalid title returns `400` and creates
  nothing. A malformed body gets axum's `4xx`.
- `PATCH /api/meetings/{meeting_id}` accepts `{"title":"..."}` and returns `200` with the
  renamed meeting's summary, `400` with `{"error":"..."}` for an invalid title (nothing is
  written), and `404` for an unknown meeting (Issue #38). It calls
  `application::meeting_rename`, the same use case as the CLI and FFI. CORS allows `PATCH`.
- `GET /api/meetings/{meeting_id}` returns `meeting_id`, `title`, `created_at`, nullable
  `transcript` (the recorded segments one per line, or the imported text; see
  `application::meeting_transcript`), `segments` with each one's speaker name and confidence
  marks (`overlapping_speech`, `uncertain_speaker`, `uncertain_words`; #90, #91), and linked
  attachment metadata (`attachment_id`, `file_path`, `mime_type`, `original_timestamp_utc`). It
  returns `404` for an unknown meeting and never returns CRDT bytes or attachment contents.
- `GET /api/voice-gallery` returns saved people with their voice sample counts, every meeting's
  unresolved clusters, and unresolved clip ID/time-range metadata. It never returns raw audio or
  voice embedding vectors.
- `GET /api/clusters/{id}/review-clip` serves a cluster's review clip for playback, uncached.
- `POST /api/voice/assign` accepts `{"clip_id":"uuid","identity_id":"uuid"}` and returns `204`
  after `VoiceIdentityManager` stores a derived signature variant. It validates both UUIDs and
  never serializes the clip's temporary raw audio or embedding.
- `/api/projects*` and `PUT /api/meetings/{id}/project` (Issue #85, `project_routes.rs`) list,
  create, rename and delete projects, move meetings, and add (upload or note), read and delete
  resources, through the commands the app uses (`bin/project_commands.rs`). File work runs on a
  blocking thread; `ServerAppState::with_project_files` attaches the folder, without which those
  routes answer 503. Resource bytes are served with `nosniff` and a sandboxing CSP. API_SPECS.md
  has the table.
- `/api/p2p/*` (Issue #43, `p2p_routes.rs`) drives the daemon's P2P task through the
  `P2pControl` port, which `ServerAppState::with_p2p_control` attaches:
  - `GET /api/p2p/status` returns this device (`local_peer_id`, `device_name`,
    `listen_addresses`, and the `capabilities` of its manifest), `trusted_devices` (each with
    its manifest's `capabilities` while connected, otherwise `null`), `nearby_devices`, `incoming_requests` and
    `outgoing_pairings` with their six-digit `code`, and `recent_updates` tagged by `kind`.
    Without P2P it returns `{"enabled":false}`.
  - `POST /api/p2p/pairable`, `POST /api/p2p/pairings` (`{"peer_id"}`, returns `{"code"}`),
    `POST /api/p2p/pairings/{peer_id}/confirm|cancel`, and
    `POST /api/p2p/requests/{peer_id}/accept|reject` carry out the pairing steps.
  - `POST /api/p2p/invites` opens the pairing window and returns `{"invite","expires_at_ms"}`.
    `POST /api/p2p/invites/join` (`{"invite"}`) lists that device as nearby and returns
    `{"peer_id"}`. Refusals are `400` with `{"error_code"}` (Issue #65).
  - `DELETE /api/p2p/devices/{peer_id}` revokes a device (`404` when it was not paired). CORS
    allows `DELETE`.
  - Errors: a malformed peer ID returns `400` without reaching the task; a refused command `409`
    with `{"error"}`; disabled or stopped P2P `503`.
  - A successful `POST /api/meetings` or `PATCH /api/meetings/{meeting_id}` reports the meeting
    through `P2pControl::notify_local_change`, so connected devices get it at once. Cluster
    assignments are not reported here: the voice library syncs by its own digest (#56).
- Settings (Issue #46, `log_routes.rs`) work through the `LogControl` port, which
  `ServerAppState::with_log_control` attaches:
  - `GET /api/settings/log-level` returns `{"level"}`, the level in effect.
  - `PUT /api/settings/log-level` with `{"level"}` applies it until the daemon restarts and
    returns `{"level"}`; an invalid directive returns `400` `{"error_code":"invalid_level"}`.
    CORS allows `PUT`.
  - `GET /api/logs/export` returns the newest seven daily log files as one
    `application/x-ndjson` attachment (`note_taker_logs_<YYYYMMDD-HHMMSS>.jsonl`), or `404`
    `{"error_code":"no_logs"}`. It writes through a per-request temporary directory that it removes.
  - Without a log control every route returns `503`. `log_routes_tests.rs` covers each case and
    the CORS preflight.
- The transcription glossary (`glossary_routes.rs`) lives in the settings file that
  `ServerAppState::with_settings_file` names:
  - `GET /api/settings/glossary` returns `{"terms":[...]}`.
  - `PUT /api/settings/glossary` with `{"terms":[...]}` saves the cleaned list and returns it;
    an invalid list returns `400` `{"error_code":"invalid_glossary"}`, a write failure `500`
    `save_failed`, and a daemon without a settings file `503` `unavailable`. Terms are never logged.
- The speech retention window (`retention_routes.rs`, #93) lives in the same file:
  - `GET /api/settings/retention` returns `{"days","min_days","max_days"}`.
  - `PUT /api/settings/retention` with `{"days"}` saves a window of 1 to 60 days and returns it;
    out of range returns `400` `invalid_window`, a write failure `500` `save_failed`, and no
    settings file `503` `unavailable`. `retention_routes_tests.rs` covers each case.
- Keeping online-meeting participants' voices (`meeting_voices_routes.rs`, #103) lives in the same
  file: `GET /api/settings/meeting-voices` returns `{"keep"}` (`false` until chosen), and
  `PUT /api/settings/meeting-voices` with `{"keep"}` saves it and returns it. A write failure
  returns `500` `save_failed`, and no settings file `503` `unavailable`.
- AI connections and summaries (`llm_routes.rs`, #95) run the commands the app uses
  (`bin/llm_commands.rs`); `start_auto_summaries` runs the daemon's automatic summaries (#101).
- The owner's edits (`edit_routes.rs`, #96): `GET`/`PUT /api/meetings/{id}/notes`,
  `PUT /api/meetings/{id}/summary` (the text, marked edited) and `PUT /api/meetings/{id}/segments`
  (one segment by start and end; blank text removes it). See API_SPECS.md.
- Search (`search_routes.rs`, #100): `GET /api/search?q=&scope=meetings|projects|project&project=`
  answers with the hits of `bin/search_commands.rs`. See API_SPECS.md.
- Chat (`chat_routes.rs`, #97): `GET`/`POST`/`DELETE /api/chat` over
  `bin/notes_chat_commands.rs`. See API_SPECS.md.
- Notes folder (`notes_folder_routes.rs`, #98): `GET`/`PUT /api/settings/notes-folder` and
  `POST /api/settings/notes-folder/sync` over `bin/notes_folder_commands.rs`;
  `start_notes_folder` runs the daemon's rounds. The folder is on the daemon's machine.
- `GET /api/telemetry` returns `task_count`, `total_duration_ms`, and `average_latency_ms`.
- `GET /api/devices` returns telemetry-observed devices with `device_id`, feature-gated
  `supports_vad`, observed `supported_models`, and epoch-millisecond `last_seen`. Device data is
  derived only from existing privacy-safe metric records.
- `GET /api/telemetry/devices/{device_id}` returns the previous 24 hours of task metrics as
  `duration_ms`, `latency_ms`, `power_mwh`, `recorded_at`, and task category. It returns `404`
  when the device has no recent metric.
- `GET /api/telemetry/summary` returns `total_transcription_time_ms`, `average_latency_ms`,
  `total_tasks`, and `power_estimate_mwh`; no raw task inputs or outputs are included.
- `POST /api/meetings/{meeting_id}/export` accepts
  `{"format":"markdown|pdf|json","options":{"include_attachments":true,"include_metadata":true}}`
  and returns the exported file as a download (`Content-Disposition: attachment`). Unknown or
  reserved formats return `400`, unknown meetings `404`, and errors are `{"error":"..."}`.
  Extraction runs under the repository lock; rendering runs on `spawn_blocking` in a per-request
  temp directory that is removed before the response is sent. See `API_SPECS.md`.
- `GET /api/meetings/{meeting_id}/unknown-clusters` returns the meeting's unresolved
  unknown-speaker clusters as `[{"cluster_id","meeting_id","total_duration_ms","chunk_count",
  "created_at"}]`. An unknown meeting returns `[]`, and embeddings are never serialized.
- `POST /api/clusters/{cluster_id}/assign` accepts `{"identity_name":"..."}` and returns `200`
  with `cluster_id`, `identity_id`, `identity_name`, `identity_created`, `variants_stored`, and
  `segments_updated`. A bad UUID or blank name returns `400`, an unknown cluster `404`, and an
  already-resolved cluster `409`. Both routes call `application::cluster_review`. See `API_SPECS.md`.
- `GET /ws` upgrades to a WebSocket. Text events are tagged JSON: `meeting_started`,
  `sync_status_changed`, `new_unassigned_clip`, or `telemetry_updated`; their payload carries only
  opaque IDs/status.

`ServerAppState::publish_event` is the local application-facing event seam. Producers send a
typed `WebSocketEvent`; the adapter serializes it for connected web clients without exposing a
storage implementation or raw content.
