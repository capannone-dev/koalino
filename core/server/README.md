<!-- Documents the headless HTTP and WebSocket adapter boundary. -->

# Headless server adapter

This directory will host the Axum adapter for the Linux headless mode. Its router receives
application-facing services and repository ports through dependency injection; handlers must never
access SQLite or other storage-specific APIs directly. Tests use in-memory SQLite plus Axum's
in-process request service so CI does not require a fixed network port.

## Runtime configuration

`daemon` binds `NOTE_TAKER_BIND_ADDRESS` (default `127.0.0.1:8080`) and opens
`NOTE_TAKER_DATABASE_PATH` (default `note_taker.sqlite3`). Keep the default loopback bind unless a
separately authenticated reverse proxy is configured. CORS permits only `http://localhost:3000`.

## HTTP contract

- `GET /api/meetings` returns `[{"meeting_id","title","created_at"}]` metadata only. `title`
  is the stored title, or `Meeting YYYY-MM-DD HH:MM UTC` for untitled meetings; `created_at` is
  RFC 3339 UTC.
- `POST /api/meetings` accepts `{"title":"..."}` or `{}` and returns `201` with the new
  meeting's summary (generated `meeting-<uuid>` ID). An invalid title returns `400` and creates
  nothing. A malformed body gets axum's `4xx`.
- `GET /api/meetings/{meeting_id}` returns `meeting_id`, `title`, `created_at`, nullable
  `transcript`, and linked
  attachment metadata (`attachment_id`, `file_path`, `mime_type`, `original_timestamp_utc`). It
  returns `404` for an unknown meeting and never returns CRDT bytes or attachment contents.
- `GET /api/voice-gallery` returns display-safe identity labels and unresolved clip ID/time-range
  metadata. It never returns raw audio or voice embedding vectors.
- `POST /api/voice/assign` accepts `{"clip_id":"uuid","identity_id":"uuid"}` and returns `204`
  after `VoiceIdentityManager` stores a derived signature variant. It validates both UUIDs and
  never serializes the clip's temporary raw audio or embedding.
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
