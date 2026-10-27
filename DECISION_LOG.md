<!-- Records architectural decisions that contributors should read before modifying the implementation. -->

# Decision log

## 2026-10-25 — Issue #1 Rust core scaffold

- Adopted the documented hexagonal layout: domain, ports, application, adapters, and binary composition roots.
- Kept AI, P2P, storage, and CRDT integrations behind traits; no heavyweight model dependencies were added.
- Selected `tracing`, `tracing-subscriber`, and `tracing-appender` for required JSON stdout and daily rolling-file logs. The filter is reloadable at runtime.
- The repository currently spells the requirements file `REQUIRMENTS.md`; links retain that existing filename to avoid an unrelated rename. Superseded on 2026-10-27: the file was renamed to `REQUIREMENTS.md` at the owner's request.

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

## 2026-10-26 — Issue #23b transcription context propagation

- Added the portable `TranscriptionContext` to the `TranscriptionEngine` boundary. It carries only
  optional immediately preceding same-meeting transcript text, never raw audio or provider values.
- `ComputeDispatcher` retains the last successful transcript per meeting in memory and supplies it
  to the next request. It does not persist, log, or send the rolling buffer over P2P.
- Local whisper.cpp maps prior text to its initial prompt; the OpenAI adapter maps it to the
  multipart `prompt` field. Adapter-specific request details remain outside application code.

## 2026-10-26 — Issue #23c WebRTC VAD chunk state machine

- The audio adapter owns a private `Idle -> Listening -> PauseDetected -> Finalize` state machine;
  raw PCM remains adapter-local until a portable finalized chunk is emitted.
- The optional `webrtc-vad` dependency, classifier instance, and frame classification call are all
  compiled only with the `vad` feature. English finalizes after 600 ms of silence and Italian after
  800 ms.
- VAD-disabled builds and VAD classification failures warn without recording samples and use bounded
  30-second time chunking, keeping offline capture available when the optional capability is absent.
- A 120-second soft maximum retains one second of adapter-owned PCM lookback for the successor
  chunk; callers receive only the existing portable chunk contract and cut type.

## 2026-10-26 — Issue #24 telemetry dashboard

- Dashboard API handlers depend only on `MetricsRepository`; all-device reads remain implemented
  inside SQLite, and responses omit task inputs, outputs, raw audio, and biometric data.
- Device cards are telemetry observations: models and last-seen time are derived from completed
  metrics, while the feature-gated server VAD capability is exposed without hardware identifiers.
- The Next.js client uses Recharts with responsive containers and coalesces `telemetry_updated`
  WebSocket refreshes to one fetch sequence per ten seconds; its existing cloud Node workflow is
  the authoritative UI verification environment.

## 2026-10-26 — Issue #25 Flutter telemetry dashboard

- Flutter receives only caller-freed device, summary, and time-series JSON; raw task content never
  crosses FFI. A controller owns ten-second polling and responsive Material 3 `fl_chart` views.

## 2026-10-27 — Issue #26 meeting export

- Transcripts were stored only as flat text, and diarization attributions were never persisted.
  Added a `transcript_segments` table behind new `NoteRepository` methods with backward-compatible
  trait defaults. Populating it from the pipeline is tracked as follow-up work. Until then, exports
  fall back to the flat transcript as one `Unattributed` segment.
- `MeetingExporter` takes extracted `ExportDocument` values, not meeting IDs, so adapters never
  read repositories. `ExportManager` keeps the ID-based API, and all three interfaces call it.
  Meeting IDs stay `MeetingId` (string) rather than the issue's `Uuid`, matching the codebase.
- `ExportManager` borrows its `ExportWriter`, so the HTTP handler extracts under the repository
  lock and renders PDF/Markdown on `spawn_blocking`, keeping the async runtime free.
- PDF uses `printpdf` 0.7 with its re-exported `image` and `lopdf`. The direct `image` dependency
  was removed, so no crate beyond `printpdf` was added. Non-image attachments are embedded through
  the `/EmbeddedFiles` name tree. Built-in Helvetica covers only WinAnsi, and unencodable
  characters render as `?` rather than vanishing.
- Markdown copies attachments into `<stem>_attachments/` beside the file for portable relative
  links. PDF staging uses a temp directory that is always removed. JSON lists original paths.
- Exports and segment text follow the existing `transcript` column's at-rest policy. Neither is
  encrypted yet, and neither ever contains embeddings or raw audio.

## 2026-10-27 — Issue #27 speaker-attributed transcript pipeline

- `MeetingSession` lives in `core/application/pipeline` and uses only existing ports. The VAD
  chunker is an adapter, so the session takes its own `VadAudioChunk` value, which carries the
  recording, meeting-relative bounds, and opaque audio.
- The transcription port returns text without word timestamps. VAD chunk bounds are therefore the
  alignment reference: regions are clamped, same-speaker neighbours merged, gaps split at midpoints,
  and words distributed by duration. This is documented as a heuristic until word timestamps exist.
- `save_transcript_segments` replaces a meeting's whole timeline. The session therefore mirrors the
  timeline in memory, seeds it from storage at start, and rewrites it per chunk. On a save failure
  the in-memory state is left unchanged.
- The segment schema was not changed. Pending-review, unknown, and unattributed segments persist
  `speaker_identity_id = None`, and the category is returned in `AttributedSegment`. Persisting the
  category is tracked as follow-up.
- Failure policy: a transcription error persists nothing and is returned. Diarization errors or an
  absent engine produce "Unattributed". Matcher errors produce "Unknown" with a review clip.
  External mapping errors produce "Unknown" with no clip. Review-clip storage errors are logged,
  and the segment is still persisted.
- The external path takes `duration_ms` explicitly because `ExternalMediaChunk` carries only a
  start timestamp. It treats the bridge timestamp as meeting-relative.

## 2026-10-27 — Issue #28 unknown-speaker clustering

- The clustering policy, including the cosine math, lives in
  `application/unknown_speaker_clusterer.rs`, not behind a port. The issue's
  `UnknownSpeakerClusterer` trait was not added because there is only one implementation and the
  logic is application policy. Persistence extends `NoteRepository` with default-implemented
  methods, following the #26 precedent, so no new port was introduced. This is a deliberate
  exception to the #9 rule that the application never computes similarity. That rule still
  applies to identity matching against the voice library.
- `find_or_create_cluster` returns `ClusterMembership { cluster_id, is_new_cluster }` rather than
  a bare `Uuid`. The session needs `is_new_cluster` to save exactly one review clip per cluster.
- Joining requires similarity strictly greater than the threshold, and the best-scoring
  qualifying cluster wins. Dimension mismatches never join. Empty, non-finite, and zero-norm
  embeddings are rejected. The representative is the running mean `(rep·n + e)/(n+1)`.
- Segments are linked to clusters through a new nullable `transcript_segments.unknown_cluster_id`,
  not inferred from time ranges, so assignment relinks exactly the right rows. The chunk ID is the
  review-clip UUID generated for the voice.
- Resolution marks the cluster resolved, keeping its row and members for provenance, and deletes
  its review clips in the same transaction, per the raw-audio PII rule. Resolved clusters never
  absorb new chunks.
- Identity "creation" means enrolling the first variant in `VoiceVectorDB`, because no port
  persists identities or display names on their own.
- Assignment across `VoiceVectorDB` and `NoteRepository` is not atomic. Variants are stored first,
  then segments, then resolution. A retry after a mid-way failure can duplicate variants but never
  resolves a cluster without relinking its segments.
- `MeetingSession::assign_identity_to_cluster` reloads the in-memory timeline after assignment,
  because the session rewrites the whole timeline per chunk.
- Cluster and member embeddings follow the existing clip-embedding at-rest policy, which is
  unencrypted. AES-256-GCM needs a crypto crate, and adding one requires approval.

## 2026-10-27 — Issue #29 cluster review interfaces

- The CLI, HTTP, and FFI adapters share one use case, `application::cluster_review`, so name
  validation and name-to-identity resolution live in one place. The clusterer itself is unchanged.
- No port persists display names, and the CLI and FFI compose `InMemoryVoiceVectorDB`. "Creates
  the identity (if new)" is therefore implemented as: reuse an identity whose
  `list_speaker_identities` name matches (trimmed, ignoring case), else a fresh UUID that the
  clusterer turns into an identity by enrolling variants. The typed name is echoed in every
  response but not stored. Persisting names needs a port change and is tracked as follow-up work.
- Added `impl NoteRepository for &T where T: ?Sized`, mirroring `VoiceVectorDB`, so services
  accept the HTTP server's `dyn ServerRepository`. It replaces the FFI-local
  `&SqliteNoteRepository` impl, which had silently used the default empty `list_meeting_ids`.
- Unknown meetings list as an empty array rather than `404`/error on every interface, matching
  `get_clusters_for_meeting`.
- HTTP maps a blank name to `400`, an unknown cluster to `404`, and an already-resolved cluster to
  `409`. FFI uses the matching `error_code` values in the same `{"ok":false,...}` shape as export.
  The issue asked for a bare JSON array from `note_taker_get_unknown_clusters`, so success is an
  array and failure is the error object. Dart distinguishes them by JSON type.
- The FFI listing adds an optional `review_clip_path` so Flutter keeps sample playback. It writes
  the clip on demand and deletes the files after assignment, per the raw-audio PII rule.
- Flutter cluster calls run in `Isolate.run` from top-level functions that reopen the core library
  in the isolate. The closures therefore capture only strings, and SQLite work stays off the UI
  isolate.
- Per the issue, the Flutter and web galleries replaced the clip and weak-match UI with clusters.
  Clips not tied to a cluster (the clustering-failure fallback) are no longer shown there, though
  their FFI and HTTP endpoints remain.
- The requirements file was renamed to `REQUIREMENTS.md`.
- Web CI had failed since #19 because vitest globals were disabled and jsdom lacks
  `ResizeObserver`. Both were fixed. The repository has no Flutter CI workflow, so Dart changes are
  verified only where a Flutter SDK is available.
