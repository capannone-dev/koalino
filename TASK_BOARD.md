<!-- Tracks the implementation status of work packages and must be consulted before future work begins. -->

# Task board

## WP1: Core Domain & Ports

| Issue | Status | Scope |
| --- | --- | --- |
| #1 Initialize Rust Core scaffolding | Ready for review | Hexagonal crate layout, typed domain models, ports, application recording use case, tracing observability, Makefile, and CI. |

## WP2: Storage & Sync Adapters

| Issue | Status | Scope |
| --- | --- | --- |
| #2 SQLite NoteRepository and Automerge CRDT integration | Ready for review | In-memory TDD coverage, SQLite meeting schema, opaque CRDT BLOB persistence, corruption handling, and offline merge service. |

## WP3: Audio Capture

| Issue | Status | Scope |
| --- | --- | --- |
| #3 AudioCapture port and local recording state machine | Ready for review | Portable PCM chunk/state subscriptions, TDD state-transition coverage, local deterministic adapter, and structured transition logging. |

## WP4: AI & Compute Routing

| Issue | Status | Scope |
| --- | --- | --- |
| #4 TranscriptionEngine trait, device manifests, and compute dispatcher | Ready for review | Trait-only transcription dispatch, non-sensitive capability manifests, deterministic local/specific/automatic routing, and mock-only TDD coverage. |
| #9 Voice identity domain, vector port, and matching policy | Ready for review | UUID-based identities and signature variants, adapter-owned vector search, conservative confidence thresholds, user-confirmed variant creation, and in-memory cosine adapter coverage. |
| #10 DiarizationEngine port and unassigned clip preservation | Ready for review | Async segment/embedding port, mock-only processor orchestration, strong attribution links, and temporary weak/unmatched review clips. |
| #12 AI task metrics, device load, and performance statistics | Ready for review | Telemetry domain/port, dispatcher timing wrapper, in-memory SQLite persistence and aggregation coverage. |
| #13 Attachment domain, metadata extraction, and smart meeting linking | Ready for review | Attachment/time-window domain values, metadata and attachment ports, mock-only midpoint confidence ranking, and deterministic suggestion ordering. |
| #15 Teams Bridge port, identity mapping, and auto-enrollment | Ready for review | Provider-neutral external media/slide ports, stable-ID local identity mapping, display-name auto-creation, automatic voice variants, and mock-only TDD coverage. |
| #16 Background recording architecture and lock-screen control hooks | Ready for review | Recoverable interruption state, mutex-serialized lifecycle/state FFI, and platform wiring guides without OS-specific source code. |
| #20 Local Whisper.cpp transcription adapter and Cargo feature flags | Ready for review | Optional Rust-1.88-compatible whisper.cpp binding, adapter-local normalized PCM seam, feature-gated real-model contract test, and mock-safe default test path. |
| #22 OpenAI Whisper configuration and cloud adapter | Ready for review | Reqwest multipart WAV adapter, API-key environment loading, provider registry/configuration, and mock HTTP coverage. |
| #23 VAD-based chunking and context-aware transcription | Ready for review | Feature-gated WebRTC VAD state machine, language-sensitive pause cuts, soft-max lookback, graceful time fallback, capability advertisement, and context propagation are implemented. |

## WP5: Flutter UI & FFI

| Issue | Status | Scope |
| --- | --- | --- |
| #5 Initialize Flutter project and Rust FFI bridge | Ready for review | Material 3 recording start UI, bridge-injected widget tests, Dart FFI adapter, and C-compatible Rust recording/log-level commands. |
| #11 Voice Gallery, unassigned playback, and weak match prompting | Ready for review | JSON metadata/path FFI, local native playback, Riverpod controller, Material 3 review screen, and fake-bridge widget tests. |
| #14 Attachment import UI, file picking, and meeting suggestion workflow | Ready for review | File-picker seam, JSON-only AttachmentManager FFI, Riverpod controller parsing, Material 3 ranked confirmation dialog, and fake-boundary widget tests. |
| #25 Flutter telemetry dashboard | Ready for review | Caller-freed telemetry FFI JSON, polling controller, responsive Material 3 fl_chart dashboard, and drawer navigation. |

## WP7: Headless server API

| Issue | Status | Scope |
| --- | --- | --- |
| #17 Axum headless REST/WebSocket API | Ready for review | Port-injected meeting/gallery/telemetry reads, local-only CORS, typed status-event WebSocket, in-memory SQLite and ephemeral-port integration tests. |

## WP8: Linux CLI

| Issue | Status | Scope |
| --- | --- | --- |
| #18 Interactive recording, meeting, voice, and telemetry CLI | Ready for review | Clap parser, injected command dispatch, shared SQLite composition, quiet terminal logging, and mock/in-memory test coverage. |

## WP9: Linux server dashboard

| Issue | Status | Scope |
| --- | --- | --- |
| #19 Next.js dashboard and voice assignment | Ready for review | App Router dashboard, typed Axum REST/WebSocket client, cloud Node CI, and application-backed voice assignment endpoint. |
| #24 Telemetry dashboard with device metrics and time-series charts | Ready for review | Privacy-safe device, series, and summary API endpoints; responsive Recharts dashboard; coalesced live refresh; and cloud UI verification. |

## WP6: P2P Network Layer

| Issue | Status | Scope |
| --- | --- | --- |
| #6 Initialize libp2p swarm, mDNS local discovery, and encrypted transport | Ready for review | Noise-encrypted TCP, Yamux, Tokio mDNS, portable network-event handling with mocked-port TDD, and no socket-based unit tests. |
| #8 Implement libp2p application payload protocol for SyncMessages | Ready for review | Private versioned request-response behaviour, bounded opaque payload framing, portable message translation, and Noise-over-memory directed/broadcast integration coverage. |

## Sync application orchestration

| Scope | Status | Details |
| --- | --- | --- |
| Automerge Sync Manager | Ready for review | Versioned sync envelopes, discovery handshakes, remote merge-and-persist flow, local encrypted broadcast requests, and mocked-port TDD for valid, local-change, and malformed-message paths. |
| Issue #21 attachment file transfer | Ready for review | Separate file-transfer behaviour, SHA-256 CAS, explicit chunk frames, mocked manager tests, and encrypted memory-transport verification. |

## Follow-up work

- Define validation and IDs suitable for CRDT replication before storage adapters are introduced.
- Add mock port implementations and expanded error-path tests under WP6.
- Implement AI adapters only in their assigned work packages.
