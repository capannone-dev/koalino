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

- `GET /api/meetings` returns `[{"meeting_id":"..."}]` metadata only.
- `GET /api/meetings/{meeting_id}` returns `meeting_id`, nullable `transcript`, and linked
  attachment metadata (`attachment_id`, `file_path`, `mime_type`, `original_timestamp_utc`). It
  returns `404` for an unknown meeting and never returns CRDT bytes or attachment contents.
- `GET /api/voice-gallery` returns display-safe identity labels and unresolved clip ID/time-range
  metadata. It never returns raw audio or voice embedding vectors.
- `GET /api/telemetry` returns `task_count`, `total_duration_ms`, and `average_latency_ms`.
- `GET /ws` upgrades to a WebSocket. Text events are tagged JSON: `meeting_started`,
  `sync_status_changed`, or `new_unassigned_clip`; their payload carries only opaque IDs/status.

`ServerAppState::publish_event` is the local application-facing event seam. Producers send a
typed `WebSocketEvent`; the adapter serializes it for connected web clients without exposing a
storage implementation or raw content.
