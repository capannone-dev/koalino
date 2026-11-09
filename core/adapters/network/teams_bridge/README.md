# Teams bridge adapter (`core/adapters/network/teams_bridge`)

Receives a Teams bot's isolated per-participant audio (or, from the browser extension's native
messaging host, the owner's microphone and the mixed remote audio), screen-share frames, meeting
documents (Teams transcript, captions), meeting ends, and meeting metadata
over a localhost WebSocket using the Protobuf protocol in `core/protocol/README.md`, and hands
them over as domain values (`ExternalMediaChunk`, `SlideCapture`, `ExternalMeetingDocument`,
`BridgeMeeting`). The bot is
a separate process: Teams SDKs and media never enter the core. The composition root that
connects this adapter to the pipeline is `core/bin/teams_bridge_composition.rs`.

| File | Purpose |
| --- | --- |
| `message_decoder.rs` | The only user of the generated Protobuf types. Validates frames (16 kHz PCM, 1–30 s, metadata before media, screen share vs. camera) and derives the local meeting UUID (v5 of the Teams meeting ID). |
| `bridge_buffer.rs` | Bounded queue between the reader and the pump. Holds `BUFFER_SECONDS` of audio and 16 slides, drops the oldest when full with a rate-limited warning, and never blocks the producer. Meeting starts, ends and documents are never dropped. |
| `teams_websocket_server.rs` | `tokio-tungstenite` server on 127.0.0.1. Refuses `Origin` headers, handles the optional shared-secret handshake (1008 on failure), keeps one active bot, and logs connects, disconnects, and reconnects. |
| `log_capture.rs` | Test-only capture of `tracing` output. |
| `*_tests.rs` | TDD specs. The server tests drive a mock `tokio-tungstenite` bot sending Protobuf frames; the decoder tests use golden `protoc` fixtures. |

## Sequence

```text
Teams bot        TeamsWebSocketServer       message_decoder    BridgeBuffer   pump thread (bin)        RecordingManager worker / MeetingSession
   │ WS connect (no Origin) ──▶│                     │                │              │                          │
   │ {"auth":"…"} ───────────▶│ constant-time check │                │              │                          │
   │◀──────────── {"auth":"ok"}│ (else close 1008)   │                │              │                          │
   │ MeetingMetadata ────────▶│ decode_frame ──────▶│ MeetingStarted ▶│ push ───────▶│ begin_external_meeting ─▶│ create/resume meeting
   │ AudioChunk ─────────────▶│ decode_frame ──────▶│ Audio ────────▶│ push (drop   │ ChunkAudioStore.insert    │
   │                          │  (never blocks)     │                │  oldest)  ──▶│ process_external_chunk ─▶│ transcribe → IdentityMapper
   │                          │                     │                │              │◀── segments (waits) ──── │ → Known segment persisted
   │ VideoFrame(screen) ─────▶│ decode_frame ──────▶│ Slide ────────▶│ push ───────▶│ SlideAttachmentStore     │ (image file + attachment row)
   │ ✕ disconnect             │ log, keep meeting, keep listening    │              │                          │
   │ WS reconnect … ─────────▶│ same flow; UUIDv5 meeting ID resumes the same meeting                          │
```

Security: loopback only, no `Origin`, optional shared secret, 8 MiB message cap. Secrets, titles,
e-mail addresses, audio, and images are never logged. Chunk audio exists only in memory and is
deleted from the `ChunkAudioStore` after transcription.
