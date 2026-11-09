# Wire protocols (`core/protocol`)

Schemas for processes that talk to the Rust core from outside: the Teams bot, and the browser
extension through its native messaging host (`browser-extension/`, Issue #102).
The `.proto` files are the source of truth. `generated/` holds the Rust code rust-protobuf
3.7.2 produces from them; it is checked in, and the `generated_code_tests` test fails if it is
stale. To regenerate after editing a schema, run
`NOTE_TAKER_REGENERATE_PROTOCOL=1 cargo test --lib protocol::generated_code_tests`.
`fixtures/` holds golden frames encoded by `protoc`, so the Rust decoder is checked against an
independent encoder.

## Teams bridge protocol v1 (`teams_bridge.proto`, package `note_taker.teams_bridge.v1`)

### Transport

- **Endpoint:** WebSocket at `ws://127.0.0.1:<NOTE_TAKER_TEAMS_BRIDGE_PORT>` (default `8765`). The
  server binds the loopback interface only, so traffic never leaves the machine.
- **Origin header:** handshakes that carry an `Origin` header are refused with HTTP 403, so web
  pages cannot connect (cross-site WebSocket hijacking).
- **Message size:** at most 8 MiB per message.
- **Encoding:** one binary WebSocket message carries one serialized `TeamsBridgeMessage`.
- **Single bot:** one bot connection is active at a time. A newer authenticated connection
  replaces the older one, which is closed with code 1000.

### Authentication

When `NOTE_TAKER_TEAMS_BRIDGE_SECRET` is set:

1. Within 5 s of connecting, the bot's first message must be the text frame
   `{"auth":"<secret>"}`.
2. The server compares the secret in constant time and replies with the text frame
   `{"auth":"ok"}`.
3. A wrong secret, a binary first frame, or silence closes the connection with code 1008 (policy
   violation). The secret is never logged.

Without a secret, any local process can connect, and start-up logs a warning. A bot may still
send the auth frame; the server answers `{"auth":"ok"}`.

### Messages (`TeamsBridgeMessage.payload`, a `oneof`)

| Message | Fields | Rules |
| --- | --- | --- |
| `MeetingMetadata` | `meeting_id`, `meeting_title`, `participants[]`, `start_time_ms` | Must come before any media. `meeting_id` must not be empty. `start_time_ms` is Unix epoch milliseconds. The title is stored by the bot only and never logged. |
| `AudioChunk` | `participant`, `audio_data`, `timestamp_ms`, `sample_rate`, `track` | One track's audio. `track` is `PARTICIPANT` (default: one participant's isolated audio, or the owner's own microphone in the browser) or `MIXED_REMOTE` (every remote participant mixed, as a browser receives a call: its voices are matched against the gallery from the first chunk, and `participant` is never credited or mapped). Format: 16-bit little-endian mono PCM. `sample_rate` must be `16000`. Contains whole samples only (non-empty, even byte count). Length 1–30 s; chunks over 30 s are rejected. `timestamp_ms` is relative to `start_time_ms`. |
| `VideoFrame` | `image_data`, `timestamp_ms`, `source` | `source` is `screen_share` (PNG or JPEG, stored as a slide attachment) or `camera` (accepted, not stored). |
| `MeetingDocument` | `kind`, `media_type`, `content`, `timestamp_ms`, `meeting_id` | A text document stored as a sealed attachment of the meeting. `kind` is `teams_transcript` (published by Teams after the meeting), `teams_live_transcript` (the transcript panel read during the meeting) or `teams_captions` (live captions). `media_type` is `text/vtt` or `text/plain`. `content` is non-empty UTF-8 of at most 4 MiB. An empty `meeting_id` means the active meeting; a Teams meeting ID names an earlier meeting directly, so a transcript published after the meeting does not reopen it. |
| `MeetingEnded` | `meeting_id` | The active meeting is over: the core closes it and its after-meeting pass runs. Must name the active meeting. |
| `ExternalParticipant` | `external_id`, `display_name`, `email` | `external_id` is the stable Teams user ID, used as the identity-mapping key. An empty `display_name` falls back to the ID. An empty `email` means none. |

### Error handling and reconnection

- An invalid frame is logged with a warning and skipped, and the connection stays open. This
  covers malformed bytes, an empty envelope, the wrong sample rate, odd or empty audio, audio
  over 30 s, an unknown video source, and media sent before metadata.
- A disconnect is logged. The server keeps listening and remembers the active meeting, so a bot
  that reconnects can resend metadata or continue sending media. The local meeting ID is
  UUIDv5(`meeting_id`), so the same Teams meeting always lands in the same local meeting.
- Buffering: up to `NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS` (default 10) of audio, summed across
  participants, waits for transcription. When the buffer is full, the oldest audio is dropped
  and a rate-limited warning is logged. Slides are capped at 16 queued frames.

### Versioning

Add new fields with new tag numbers, and never reuse or renumber existing ones. Incompatible
changes get a new package (`...v2`).
