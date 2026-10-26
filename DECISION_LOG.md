<!-- Records architectural decisions that contributors should read before modifying the implementation. -->

# Decision log

## 2026-10-25 — Issue #1 Rust core scaffold

- Adopted the documented hexagonal layout: domain, ports, application, adapters, and binary composition roots.
- Kept AI, P2P, storage, and CRDT integrations behind traits; no heavyweight model dependencies were added.
- Selected `tracing`, `tracing-subscriber`, and `tracing-appender` for required JSON stdout and daily rolling-file logs. The filter is reloadable at runtime.
- The repository currently spells the requirements file `REQUIRMENTS.md`; links retain that existing filename to avoid an unrelated rename.

## 2026-10-25 — Issue #2 SQLite and Automerge storage

- Added `rusqlite` with its bundled SQLite feature so builds do not depend on a host SQLite installation, and added `automerge` for offline conflict-free note state.
- Extended `NoteRepository` with meeting creation plus opaque CRDT document load/save operations. The port exposes no SQLite or Automerge type.
- SQLite persists a meeting's timestamp, acquisition device, optional transcript, and Automerge bytes. Orphan writes are rejected instead of implicitly creating a meeting.
- The application CRDT service owns Automerge document creation, text replacement, deserialization validation, and peer-document merging. Corrupt BLOBs resolve to `StorageError::Rejected`.

## 2026-10-25 — Issue #3 portable audio capture scaffold

- Added a synchronous `AudioCapture` port with standard-library receiver subscriptions, avoiding a runtime, OS audio API, or P2P transport dependency at the application boundary.
- `LocalRecorder` is a deterministic adapter scaffold with `Idle`, `Recording`, `Paused`, `Stopped`, and reserved `Error` states. `Stopped` is terminal for one session instance; consumers create a new recorder to record another session.
- Audio chunks are typed signed 16-bit PCM values and are delivered only while recording. Transition logs contain state only and never audio samples.

## 2026-10-25 — Issue #4 transcription compute routing

- Retained `TranscriptionEngine` as the sole AI boundary for the dispatcher; no application code names or initializes Whisper, an API SDK, or a P2P implementation.
- Added portable `DeviceManifest` values containing non-sensitive capacity metadata and advertised `TranscriptionModel` capabilities. P2P broadcast is intentionally deferred until an encrypted libp2p adapter exists.
- `Automatic` selects the compatible target with the highest deterministic score, prioritizing GPU VRAM then CPU cores. `LocalOnly` and `SpecificPeer` do not silently reroute a user-selected workload.
- Tests use a deterministic mock transcription adapter and call counts to prove route selection without model downloads or inference.

## 2026-10-25 — Issue #5 Flutter bridge initialization

- Flutter presentation and controllers depend on the Dart `RecordingBridge` interface only;
  `RustRecordingBridge` is the single owner of `dart:ffi` calls. This preserves the hexagonal
  boundary by keeping Rust domain structs, adapter types, and ownership details out of Dart.
- The Rust crate emits a `cdylib` and exposes `note_taker_start_recording` plus
  `note_taker_set_log_level` from `core/bin/ffi.rs`. Both return documented `int32_t` status codes
  rather than exceptions, pointers, or serialized Rust values.
- The C bridge owns a process-local `LocalRecorder`; it uses the existing 16 kHz mono portable
  configuration and delegates state rules to the `AudioCapture` port. Logging is initialized only
  on a successful non-empty filter request, preserving runtime verbosity control.

## 2026-10-25 — Issue #6 libp2p encrypted local discovery

- Added a libp2p adapter that configures an ephemeral Ed25519 identity, TCP, Noise, Yamux, and
  Tokio mDNS. The adapter does not bind/listen by itself; a future composition root drives the
  swarm and chooses listener addresses.
- Added the adapter-neutral `NetworkEvent` contract and application `NetworkEventHandler`. The
  handler is tested exclusively against a mock `P2pNetwork`, keeping libp2p event types and socket
  operations out of unit tests and the application layer.
- The adapter maps an authenticated libp2p `PeerId` into the existing opaque `DeviceId` value at
  the boundary. This is a temporary identity mapping until device registration binds stable user
  device IDs to peer keys.
- `send_encrypted` rejects payloads by policy because transport initialization is not a data
  protocol. CRDT, manifest, and compute payload framing remain future work and must be explicitly
  specified before transmission is enabled.

## 2026-10-25 — Sync Manager Automerge protocol orchestration

- Defined `SyncMessage` as a versioned, length-delimited binary envelope containing a target
  meeting ID and opaque Automerge sync bytes. Malformed framing and malformed Automerge messages
  are rejected and logged without retaining note content or payload bytes.
- `SyncManager` owns a `MeetingCrdtService`, a `P2pNetwork`, and per-peer Automerge sync state.
  It initiates peer-specific handshakes on discovery, applies and persists remote changes, and
  generates required acknowledgement messages.
- Local changes use a fresh Automerge sync state and `broadcast_encrypted`, favouring safe
  offline-peer convergence over retained per-peer delivery state for the initial broadcast.

## 2026-10-25 — Issue #8 libp2p SyncMessage payload protocol

- Added the adapter-private `/note-taker/sync-message/1` request-response behaviour with a bounded
  length-prefixed codec for opaque application payloads. The protocol rides existing authenticated
  Noise-encrypted libp2p connections; it does not add plaintext networking or expose libp2p types
  above `core/adapters/network`.
- Inbound requests become only `NetworkEvent::MessageReceived` at the hexagonal boundary. Directed
  sends require an authenticated connection; broadcasts queue one request per currently connected
  peer and retain the existing offline-first application responsibility for later sync.
- An adapter-local test constructs two Noise/Yamux swarms over `MemoryTransport` and verifies both
  directed and broadcast payloads arrive as portable message events without binding TCP or UDP.

## 2026-10-25 — Issue #9 voice identity matching policy

- Added UUID-keyed `SpeakerIdentity` and `VoiceEmbedding` variants. Voice embeddings contain only
  model-derived vectors; raw audio remains outside the domain and port contract.
- Defined `VoiceVectorDB` as the sole similarity-search boundary. The application receives scored
  candidates but never knows or implements cosine similarity.
- `VoiceIdentityManager` treats scores above 0.80 as strong, scores from 0.50 through 0.80 as weak
  confirmation candidates, and lower or absent scores as no match. Confirmed weak matches create a
  fresh signature variant UUID under the selected identity.
- Added `InMemoryVoiceVectorDB` with a simple cosine loop solely for deterministic tests and local
  development. It has no persistence or at-rest encryption and must not become the production store.

## 2026-10-25 — Issue #10 diarization orchestration and review clips

- Replaced the unused synchronous diarization-label placeholder with an async `DiarizationEngine`
  port returning portable time ranges and derived speaker embeddings. No Pyannote, SpeechBrain, or
  other model dependency is coupled to the Rust core.
- Added `VoiceIdentityMatcher` as the processor's narrow matching boundary. `VoiceIdentityManager`
  implements it over the existing `VoiceVectorDB`, so diarization orchestration never knows vector
  math or concrete storage.
- `DiarizationProcessor` returns only strong segment-to-identity links. Weak and absent outcomes
  create `UnassignedVoiceClip` records containing an opaque temporary audio payload and range for
  later user review; structured logs contain only safe meeting and clip UUID metadata.

## 2026-10-25 — Issue #11 Voice Gallery FFI and local review playback

- Flutter receives an owned JSON array containing review-clip metadata, optional weak candidate,
  and absolute local paths only. Raw audio and biometric vectors never cross the FFI boundary.
- The Rust FFI facade materializes temporary local review files, delegates retrieval to
  `NoteRepository`, and delegates confirmation/new variant storage to `VoiceIdentityManager`.
- Deferred review clips persist the segment's derived embedding and weak candidate in SQLite so
  confirmation uses the original application-policy context instead of recomputing similarity in
  Flutter. The data is never logged.
- `VoiceGalleryController` is provided through Riverpod while widgets continue to depend on an
  injectable Dart bridge. `audioplayers` opens only the returned local file path.

## 2026-10-25 — Issue #12 AI telemetry and device load tracking

- Added `MetricsRepository` as the adapter-neutral persistence boundary for completed AI task
  metrics, aggregates, and latest device-load snapshots. It carries no task payload, raw audio,
  transcript content, or embeddings.
- `ComputeDispatcher` measures only the injected engine call and records successful transcription
  completions through the optional port. This preserves engines as result-producing adapters with
  no telemetry coupling.
- The first power heuristic uses advertised GPU VRAM to choose 25 W versus 5 W CPU-only and
  converts elapsed milliseconds to mWh. It is intentionally a coarse planning estimate.
- SQLite stores telemetry in dedicated tables and aggregation runs in SQL. Tests use in-memory
  SQLite and deterministic mock engines only.

## 2026-10-25 — Issue #15 external bridge guaranteed identity

- Kept provider integration outside the core through `MeetingBridge`; its standard-library
  receivers publish portable `ExternalMediaChunk` and `SlideCapture` values only. No Graph, Azure,
  bot SDK, token, or network type crosses the hexagonal boundary.
- Stable `ExternalParticipant.external_id` is the sole durable mapping key. `TeamsIdentityMapper`
  creates a UUID local `SpeakerIdentity` named from `display_name` only when the mapping is unseen;
  existing mappings preserve their local name.
- Isolated external-speaker audio bypasses probabilistic diarization and weak-match review. The
  processor derives a vector through a port and stores a new `VoiceEmbedding` variant automatically
  before forwarding media to the transcription port. All behavior is covered by mocked channels,
  mapping storage, vector storage, transcription, and attachment workflow ports.

## 2026-10-25 — Issue #16 background recording and interruption recovery

- Made OS stream suspension an explicit portable `Interrupted` audio state rather than an Error or
  terminal Stop. The interruption callback returns `AudioError::Interrupted` after safely halting
  routing; Start is the sole recovery transition and repeated callbacks are idempotent.
- Retained FFI ownership in the process-wide `OnceLock<Mutex<Option<FfiRuntime>>>`. Every
  background/lock-screen lifecycle command obtains that mutex, accepts no retained pointer, and
  returns a stable integer result. State values use `10`–`15` to avoid collision with FFI errors.
- Kept Android foreground-service, Windows runner, and Linux systemd/logind work as documented
  platform wiring. No Java, Kotlin, C#, OS service SDK, or wake-lock dependency enters the core.

## 2026-10-25 — Issue #13 attachment metadata and meeting linking

- Added `MetadataExtractor` as the sole attachment timestamp boundary. `AttachmentManager` sees
  neither EXIF/PDF APIs nor filesystem metadata and tests use hardcoded timestamps only.
- Preserved the established `MeetingId` string type for attachment links and suggestions; attachment
  records themselves receive UUID identifiers. This prevents a new subsystem from splitting the
  repository's existing meeting-identity contract.

## 2026-10-25 — Issue #14 attachment import FFI and confirmation UI

- Chose caller-freed UTF-8 JSON for attachment suggestions. Every candidate repeats a single
  opaque import UUID so Flutter parses only Dart view values and returns `(attachment_id,
  meeting_id)` for explicit confirmation; no nested Rust struct crosses FFI.
- The FFI suggestion command composes and directly delegates ranking to `AttachmentManager`.
  The local adapter reads filesystem modification time as a portable fallback capture timestamp;
  format-specific EXIF/PDF adapters can replace it without changing application policy.
- SQLite persists attachment metadata only after the user selects a meeting. It stores no file
  bytes, and Flutter exposes file selection behind an injectable `file_picker` adapter so widget
  tests do not open native dialogs or load the Rust library.

## 2026-10-25 — Issue #17 headless Axum API

- Kept HTTP and WebSocket concerns in `core/server`, injected through a mutex-protected aggregate
  of existing repository ports plus a voice-vector port. Route handlers have no SQLite imports,
  SQL, CRDT decoding, or raw-media access.
- Chose Axum `oneshot` tests for REST and an ephemeral `127.0.0.1:0` listener only for the true
  WebSocket upgrade test. This proves the wire contract while avoiding fixed-port CI collisions.
- The WebSocket broadcaster sends typed, tagged JSON status events. It deliberately omits raw
  audio, embeddings, transcript mutation, and sync payloads.

## 2026-10-25 — Issue #18 interactive clap CLI

- Kept clap parsing and terminal formatting in `core/cli`, an injected driving adapter. The binary
  contains only the shared-local-database composition root and command dispatch; it has no SQL or
  voice-matching policy.
- `voice assign` searches only through `NoteRepository` and delegates the selected clip's derived
  embedding to `VoiceIdentityManager::confirm_weak_match`. This preserves the existing signature
  persistence policy and avoids exposing audio to CLI output.
- CLI logging is fixed at `error` to protect predictable terminal output while retaining the
  project's structured logging path for failures.

## 2026-10-25 — Issue #19 Next.js local dashboard

- Chose a standalone Next.js App Router project with typed Axum DTO clients and no direct database
  access. The dashboard's server pages fetch current local state while its client components own
  WebSocket status and explicit voice-assignment interaction.
- Added `POST /api/voice/assign` as a JSON-only adapter that validates opaque UUIDs, finds clips
  through `NoteRepository`, and delegates variant storage to `VoiceIdentityManager`.
- Cloud CI installs Node 22 and runs lint, component tests, and production build. It is the source
  of web verification because the local sandbox has no Node runtime.

## 2026-10-25 — Issue #20 local whisper.cpp transcription adapter

- Added `local-ai` as an empty-by-default Cargo feature whose only optional dependency is
  `whisper-rs` 0.16.0. Its supported native binding requires Rust 1.88, so the crate MSRV moved
  from 1.85 to 1.88; native whisper.cpp building and model initialization still occur only when
  local AI is explicitly requested.
- Kept raw audio adapter-owned through `RecordingPcmSource`, which resolves only normalized 16 kHz
  mono float PCM for portable `AudioRecording` metadata. Neither that source nor whisper.cpp types
  enter the port or application layer.
- `LocalWhisperCppTranscriptionEngine` serializes one native model context, creates a fresh
  inference state per call, suppresses native progress/realtime output, and translates all native
  failures to the established portable `TranscriptionError` values.
- The `local-ai` test loads a real local model only if `NOTE_TAKER_WHISPER_MODEL` is explicitly
  configured. Default mock tests never read model paths, load models, or download model weights.

- `NoteRepository` supplies portable inclusive `MeetingWindow` values. The manager ranks only those
  windows with linear midpoint proximity: 1.0 at midpoint, 0.0 at either edge, descending score
  plus meeting-ID tie-break. Suggestions require user confirmation before attachment persistence.

## 2026-10-26 — Issue #21 P2P attachment file transfer

- Attachment bytes use a dedicated `/note-taker/file-transfer/1` behaviour; CRDT SyncMessage bytes
  remain on their existing independent behaviour.
- Explicit 64 KiB `{ hash, total_size, chunk_size, offset, is_final }` frames permit deterministic
  reassembly and reject inconsistent, short, oversized, or hash-mismatched streams.
- `FileSyncManager` uses port-only availability, authorization, and SHA-256 CAS coordination so a
  transfer failure remains best-effort work and cannot delay text synchronization.

## 2026-10-26 — Issue #22 cloud transcription adapter

- Kept OpenAI HTTP/multipart types inside `core/adapters/ai`; the application sees only
  `TranscriptionEngine`, `AiConfig`, and `AiRegistry` provider values.
- The adapter resolves a WAV source, uses `audio_chunk.wav` and `audio/wav`, and rejects uploads
  above 25 MiB before any request. `OPENAI_API_KEY` is adapter-private and never logged.
- Anthropic remains a configuration reservation for future text tasks; explicit transcription
  selection returns the portable `ProviderUnsupported` error.
