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
  `ResizeObserver`. Both were fixed. A Flutter CI workflow (`flutter-ci.yml`, pinned to Flutter 3.47.8)
  was then added. Its first runs required applying `dart format` across `ui/`, refreshing the stale
  `pubspec.lock`, and fixing a #25 type error in `telemetry_dashboard_screen.dart`.

## 2026-10-27 — Issue #30 persisted speaker identities

- Identity names live behind `VoiceVectorDB`, not `NoteRepository` and not a new port. The voice
  port already owned identities (`list_speaker_identities`, `delete_identity`), while
  `NoteRepository` holds meeting data and already has 17 methods. Splitting names from variants
  across two ports would also lose the single-transaction delete of a name and its variants. The
  port gained `save_speaker_identity` (insert or rename by ID) and `load_speaker_identity`. Both
  have defaults (`Unavailable`; a scan of `list_speaker_identities`) so search-only test doubles
  stay small.
- IDs stay `Uuid` rather than the issue's `&str`, matching every other identity API (strict typing).
- `SqliteVoiceVectorDB` adds `speaker_identities(id, display_name, created_at, updated_at)` and
  `voice_variants(id, identity_id → speaker_identities, embedding, created_at)` to the database
  file the note repository already uses, through a second connection with a 5 s busy timeout.
  The migration is `CREATE TABLE/INDEX IF NOT EXISTS` and runs on every open. No earlier release
  persisted identities or variants, so there is nothing to convert, and existing meeting, cluster,
  and segment rows are untouched. A test opens a pre-#30 database twice and checks that they survive.
- The store enforces the foreign key itself: `store_embedding` inserts only when the identity row
  exists and otherwise returns `Rejected("… does not exist")`. Callers that create identities
  therefore save them first. The clusterer does this on assignment, `TeamsProcessor` for an unseen
  participant, and FFI `assign_new_identity`, which used to drop the typed name. Confirming a weak
  match against an unknown identity UUID now fails instead of conjuring a nameless identity.
  `InMemoryVoiceVectorDB` keeps its lenient behaviour for unit tests.
- `assign_identity_to_cluster` now takes a `SpeakerIdentity`, saves it, reloads it, and returns the
  persisted identity in `ClusterAssignment`. Interfaces therefore report the stored spelling.
  `identity_created` now means "no voice-store record existed before"; before #30 it meant "no
  variants existed".
- Deleting an identity removes its name and variants in one transaction. Transcript segments that
  referenced it keep the dangling UUID and render as an unknown speaker; nothing cascades into
  meeting data.
- The CLI, daemon, and FFI now compose `SqliteVoiceVectorDB` on `NOTE_TAKER_DATABASE_PATH`.
  `InMemoryVoiceVectorDB` remains for tests only.
- Embeddings are stored as unencrypted little-endian `f32` BLOBs, the same at-rest policy as the
  #28 cluster embeddings. CONTRIBUTING.md requires AES-256-GCM for voice signatures, but that needs a
  crypto crate and therefore approval. It remains the tracked follow-up the issue itself defers.
- No FFI function lists identities, so the FFI test checks persistence by reusing a name across
  two runtimes opened on the same file.

## 2026-10-27 — Issue #31 voice embeddings encrypted at rest

- Scope: all four embedding columns, not only `voice_variants`. The issue's context names cluster
  representatives too, and cluster members and review-clip embeddings are the same biometric
  data. Covered: `voice_variants.embedding`, `unknown_speaker_clusters.representative_embedding`,
  `cluster_chunks.derived_embedding`, and `unassigned_voice_clips.derived_embedding`.
- Hexagonal boundary: AES-GCM and key handling live in `core/application/crypto/`. The adapters
  depend only on the new `EmbeddingCipher` port (`core/ports/crypto.rs`) and receive an
  `Arc<dyn EmbeddingCipher>`, so they never import application code. The metadata record goes
  through a second port, `EncryptionMetadataRepository`, implemented by
  `SqliteEncryptionMetadataStore`.
- Scheme: AES-256-GCM (`aes-gcm` 0.10) with a fresh OS-random 96-bit nonce per value, stored as
  `"NTE1"` ‖ nonce ‖ ciphertext ‖ tag. The header and a row context (`table:row-id`) are associated
  data, which goes beyond the issue's `encrypt_embedding(embedding, key)` signature (kept as the
  plain form): a value moved to another row fails to decrypt.
- Keys: Argon2id (`argon2` 0.5; 19 MiB, 2 passes, 1 lane) from `NOTE_TAKER_ENCRYPTION_PASSPHRASE`,
  otherwise a random 32-byte key in `~/.note_taker/encryption.key` (override with
  `NOTE_TAKER_ENCRYPTION_KEY_DIR`), created 0600 in a 0700 directory and never overwritten. The
  tradeoff (a key file next to the data only protects a leaked database file) is documented in
  `core/application/crypto/README.md`.
- Deviations from the issue, approved before implementation:
  - The salt and Argon2 parameters live in the database (`encryption_meta`), not in
    `~/.note_taker/encryption.salt`. The salt is not secret, and keeping it with the data means
    database plus passphrase always suffices; a lost salt file would make data unrecoverable.
  - `NOTE_TAKER_ENCRYPTION_ENABLED` is not implemented, because CONTRIBUTING.md requires encryption of
    biometrics. There is no plaintext write path; even in-memory stores use an ephemeral key.
  - No interactive passphrase prompt: Flutter and the daemon cannot answer one.
  - `rand` was not added; `aes-gcm` re-exports the OS RNG used for nonces, salts, and keys.
    `zeroize` was added to wipe keys, passphrases, and plaintext buffers.
- Key verification: `encryption_meta` stores a key-check value (a known plaintext sealed with the
  key). `storage_composition::open_encrypted_stores` unlocks and verifies the key before either
  adapter opens. A wrong passphrase, a deleted key file (no replacement is generated), or a
  switch between passphrase and key-file mode therefore stops start-up before any row is read or
  migrated. Re-keying is not supported yet. When two processes initialize at once, the loser of
  the insert adopts the stored record.
- Migration detection: the issue's "first 12 bytes look like a nonce" heuristic cannot work (any
  12 bytes are a valid nonce, and plaintext and ciphertext lengths are both multiples of 4).
  Each embedding table instead gains `embedding_format` (`0` plaintext, `1` sealed v1), added
  with default `0`, so pre-#31 rows are identified exactly. They are sealed in one transaction
  per table on open. Only format `0` is migrated; a row that already opens with the key is only
  re-marked; reads always decrypt and never accept plaintext.
- Performance: a 256-dimension embedding seals and opens in microseconds. The benchmark test
  requires 1000 round trips in under 1 s in the unoptimized test build. Argon2id runs once per
  process at start-up (about 0.6 s in a debug build).
- Fixed along the way: three FFI tests called the production `FfiRuntime::new()`, which wrote to
  the repo-root developer database and, with encryption, created `~/.note_taker/encryption.key`.
  They now use the in-memory runtime. The note repository also gained the voice store's 5 s busy
  timeout, because opening can now write.
- Follow-ups: store the key in the platform keychain (Keychain, Keystore, Secret Service), and
  support re-keying or a passphrase change.

## 2026-10-27 — Issue #32 recording flow wired into the meeting pipeline

- Composition root: `application/pipeline/pipeline_factory.rs` owns the ports once and opens a
  `MeetingSession` per recording that borrows them, through new `&T` forwarding impls for
  `TranscriptionEngine`, `DiarizationEngine`, and `VoiceIdentityMatcher`. Only existing ports are
  used. The "clusterer" dependency is the factory's `ClusteringConfig`, because
  `UnknownSpeakerClusterer` is created per chunk over the session's repository.
- Threading: the ports are not `Send` (e.g. `ComputeDispatcher` holds `Box<dyn
  TranscriptionEngine>`), so `RecordingManager` builds the factory on its worker thread from a
  `Send` closure instead of receiving engines. Graceful shutdown follows from the FIFO channel:
  `end_recording` is queued after every earlier chunk.
- Audio path: engines read audio through adapter-side sources that had no production
  implementation. `ChunkAudioStore` implements both and deletes each chunk on read. Chunk
  offsets subtract the soft-max lookback overlap. Overlapping text across a 30 s or 120 s cut is
  not yet de-duplicated (`deduplicate_overlap` exists but needs word timestamps to place
  segments), so about one second of words can repeat at those cuts.
- Approved scope decisions:
  1. No microphone adapter: the FFI gains `note_taker_push_audio_pcm`, and the CLI reads a WAV
     file or raw stdin (`arecord | note-taker record start --input -` covers live capture on
     Linux). A real capture adapter needs approval for `cpal` or a Flutter audio package.
  2. tokio's `signal` feature was enabled (existing crate) for Ctrl+C and SIGTERM.
  3. `record stop` across processes uses `<db>.recording-active` and `<db>.recording-stop`
     files; the earlier in-process CLI recorder state did not survive between invocations.
  4. No diarization adapter exists, so real recordings produce `Unattributed` segments.
- Engine selection: `auto` prefers a local model and falls back to OpenAI only when
  `OPENAI_API_KEY` is set, with a warning that audio leaves the device. Telemetry through
  `ComputeDispatcher` is not wired into the live path yet.
- FFI: the pipeline is composed on the first start, not when the runtime is created, so
  non-recording calls work without an engine. A failed composition returns the new status `4`
  and is retried. Stop holds the runtime mutex while the last chunks are transcribed, so a
  lock-screen state query waits during that time; moving the drain outside the lock is a
  follow-up.
- `vad` feature: `webrtc_vad::Vad` wraps a raw pointer and is not `Send`. A private `SendableVad`
  newtype implements `Send` (not `Sync`), with a SAFETY note: the libfvad instance is
  exclusively owned, freed in `Drop`, has no thread-local state, and is used only through `&mut`.
- Daemon: it composes the pipeline at start-up and logs its availability, but no HTTP recording
  endpoint exists yet; a missing engine is a warning, not a start-up failure.
- Follow-ups: microphone capture adapter, diarization adapter, overlap text de-duplication,
  dispatcher telemetry for live chunks, Flutter UI for push/stop, an HTTP recording API, and
  releasing the FFI lock during stop.

## 2026-10-27 — Issue #33 Teams bot ↔ Rust protocol (WebSocket + Protobuf)

- User decisions:
  1. **Protobuf library:** rust-protobuf 3.x (`protobuf = "=3.7.2"`). The `protobuf` 4.x crate
     is Google's upb-based runtime, so the version is pinned.
  2. **Chunks:** the bot sends 1–30 s chunks, and the core rejects longer ones instead of
     re-chunking.
  3. **Adapters:** both production adapters are built, a SQLite `ExternalIdentityRepository`
     and slide storage.
  4. **Authentication:** a first text frame `{"auth":"<secret>"}`, answered with
     `{"auth":"ok"}`, or the connection is closed with 1008.
- Code generation: the generated Rust code is checked in under `core/protocol/generated/`, and
  `protobuf-codegen` is a dev-dependency used by a freshness test
  (`NOTE_TAKER_REGENERATE_PROTOCOL=1` rewrites it). It is not a build script.
  - The build-script version failed `make rust-lint` locally. `protobuf-codegen` adds full
    `regex` features to the host build graph, which changes `bindgen`'s hash and forces a
    whisper.cpp rebuild that needs cmake.
  - Checking the code in also keeps code generation out of mobile cross-builds.
- Wire details:
  - Package `note_taker.teams_bridge.v1`.
  - Audio timestamps are relative to `MeetingMetadata.start_time_ms`, and metadata must precede
    media.
  - Only `screen_share` frames become slides.
  - The local meeting ID is UUIDv5 of the Teams meeting ID, so reconnects and restarts land in
    the same meeting.
- Security:
  - The server binds 127.0.0.1 only.
  - Handshakes with an `Origin` header are refused, which blocks browser-based cross-site
    WebSocket hijacking.
  - The secret is compared in constant time over SHA-256 digests and is never logged. The
    `BridgeSecret` `Debug` output is redacted.
  - Titles, e-mail addresses, and media are never logged.
  - Messages are capped at 8 MiB.
  - With no secret configured, start-up logs a warning.
- Backpressure:
  - The reader pushes into `BridgeBuffer`, which never blocks. When full, it drops the oldest
    audio (by duration, across participants) or the oldest slide (cap 16), with a warning at
    most every 5 s.
  - The pump waits for each `process_external_chunk` result, so transcription speed sets the
    pace.
  - `MeetingBridge` (unbounded std receivers) was not used for this reason.
- Pipeline: `RecordingManager` holds one external meeting session next to the local recording,
  on the same worker and models.
  - `ChunkAudioStore` gained `discard`/`discard_with_prefix`. `RecordingController::stop`
    therefore wipes only its own chunks instead of `clear()`ing audio that a concurrent Teams
    chunk still needs.
  - The controller now holds the manager in an `Arc` so the pump can share it.
- Identity: `ExternalIdentityRepository` is implemented on `SqliteVoiceVectorDB`, with a new
  `external_identity_mappings` table joined to `speaker_identities` so renames apply.
  `ports::external_bridge` gained a `Mutex<R>` forwarding impl because the mapper is shared
  with the worker thread.
- Slides: `SlideAttachmentStore` writes PNG/JPEG files (0600) under `<db dir>/slides` (or
  `NOTE_TAKER_TEAMS_BRIDGE_SLIDE_DIR`) and links them as meeting attachments.
  - They are not encrypted, the same policy as manual attachments.
  - The attachment timestamp is the capture's arrival time, because `SlideCapture` carries no
    meeting start.
- Test support: a test-only `log_capture` installs one global subscriber that routes events to
  the emitting thread's buffer. Scoped thread-local subscribers raced with `tracing`'s global
  callsite-interest cache under parallel tests.
- Follow-ups:
  - The bot itself.
  - A manual Python mock bot was not added, because Python is not in the approved stack; the
    Rust mock-bot tests cover the same path.
  - Persisting the meeting title from metadata.
  - Encrypting slide files at rest.
  - Exposing bridge status over HTTP.

## 2026-10-27 — Issue #34 C# Teams bot (audio/video extraction and streaming)

- User decisions:
  1. **Media platform:** the Graph Communications calling SDK with application-hosted
     Real-Time Media (`Microsoft.Graph.Communications.Calls.Media` 1.2.0.18736). It is the
     only supported way to get unmixed per-speaker audio and raw VBSS frames. ACS was rejected
     for media. Runtime is Windows Server x64 only.
  2. **Layout:** multi-project.
     - `TeamsBot.Core` is portable and holds all logic and tests.
     - `TeamsBot.Media` is the SDK adapter.
     - `TeamsBot` is the ASP.NET Core host.
  3. **JPEG:** SkiaSharp (MIT) encodes the JPEGs.
  4. **Dependencies approved:** Google.Protobuf with Grpc.Tools codegen from the shared proto,
     MSAL, the Graph Communications SDK, Serilog, and xUnit + NSubstitute. The CI is a
     Linux/Windows matrix.
- Wire compatibility: the C# code is generated at build time from
  `core/protocol/teams_bridge.proto`, which is linked, never copied. Tests compare the
  serializer byte for byte with `core/protocol/fixtures/*.bin`.
- Authentication:
  - MSAL client credentials, behind `CachingAccessTokenProvider`.
  - `UnauthorizedRetryHandler` re-sends a request once after a Graph `401`.
  - Inbound callbacks are validated by the SDK's built-in provider (via `SetAuthentication`),
    not by hand-written JWT code. The obsolete `SetAuthenticationProvider` path was dropped.
- Recording compliance: `updateRecordingStatus(recording)` is called before any media is
  processed. If it fails, the bot hangs up instead of recording silently.
- Audio:
  - The socket asks for `Pcm16K`, but `PcmConverter` still normalizes any rate or channel
    count.
  - Chunks end at a 600 ms silence gap or at 10 s (the Rust limit is 30 s).
  - Offsets are measured from the moment the call is established, which is also the
    metadata's `start_time_ms`.
- Roster:
  - The bot identifies participants by Azure AD object ID, falling back to
    `teams-participant-<id>`.
  - Metadata is re-sent on roster changes. This is safe because the Rust
    `begin_external_meeting` resumes a meeting with the same ID.
- Transport:
  - Plaintext `ws://` is allowed only to loopback (CONTRIBUTING.md: never plaintext over a network).
  - The documented cross-host path is an SSH reverse tunnel, because the Rust server binds
    127.0.0.1.
- Shared dependency: `Microsoft.Extensions.Logging.Abstractions` is pinned centrally at 9.0.7,
  because `Microsoft.Skype.Bots.Media` requires ≥ 9.0.7 and a lower pin fails restore
  (NU1605).
- Known limitation: the protocol has no acknowledgements. A frame in flight during a
  disconnect can be lost or duplicated. This is documented, not solved.
- Follow-ups:
  - Run the real-meeting test on a Windows host.
  - Support multiple concurrent meetings.
  - Add camera video, which is ignored by the Rust side today.

## 2026-10-27 — Issue #35 Meeting titles

- Model: there is no `Meeting` struct, so the title lives on `MeetingMetadata`, which every
  entry point already loads. `MeetingTitle` validates at the boundary: it is trimmed, 1–200
  characters, and has no control characters, so it is terminal- and export-safe.
- Migration: titles are filled in on read, not back-filled. Existing rows stay `NULL`, and
  `display_title()` derives `Meeting YYYY-MM-DD HH:MM UTC` from `created_at_utc`. The time is
  included (the issue's example was date-only) so that same-day meetings can be told apart. A
  stored fallback would go stale and could not be distinguished from a real title.
- Port: `save_meeting_title` is a separate method with a default `Unavailable`, rather than a
  new `create_meeting` parameter. This keeps every test double unchanged and serves as the
  future rename primitive.
- Recording: the title is written through `RecordingManager::with_note_repository` right after
  the worker opens the session. An untitled start never clears an existing title.
- FFI: the existing `note_taker_start_recording*` ABI is unchanged, and the new
  `note_taker_start_recording_with_title` takes nullable arguments. The issue named
  `note_taker_get_meetings`/`note_taker_get_meeting`, but they did not exist, so they were added.
- HTTP: the issue's `POST /api/meetings` did not exist, so it was added (create-only, generated
  ID). The server previously had no meeting mutation endpoints.
- Export filenames: they use the stored title, and otherwise keep the meeting ID. The dated
  fallback would repeat the filename's date prefix and drop the only distinguishing part.
  Headers always use the display title.
- Teams: the metadata title is stored only when the meeting is untitled, because metadata is
  re-sent on every roster change.
- Not replicated: titles are SQLite metadata, not CRDT content, so peers do not see them yet.
- Follow-ups:
  - CRDT replication of titles.
  - A rename command.
  - A Dart binding and UI.

## 2026-10-28 — Issue #36 Flutter meeting titles

- Bindings: they live in `ui/lib/infrastructure/` (the issue named a non-existent
  `ui/lib/ffi/note_taker_bindings.dart`).
  - `startRecordingWithTitle(String? title)` sends null for both a blank title and the meeting
    ID, so Rust generates the ID and the fallback title.
  - The issue's meeting-query bindings did not exist and were added as `RustMeetingBridge`.
- FFI extension: `note_taker_get_meetings`/`note_taker_get_meeting` gained `duration_ms` and
  `segments`. Recordings store only segments, so without them every recorded meeting's detail
  would read "No transcript". Participant counts are not exposed. The list shows date and
  duration only.
- Fallback title: Rust always sends a display title. Dart rebuilds the identical fallback only
  defensively, for null or blank values.
- Navigation: the drawer was replaced by `HomeShell`, because its push used a context above the
  `Navigator`, and the inner `Scaffold` hid its menu button.
  - 840 px (Material 3 "expanded") is the single breakpoint for both the rail and the
    list-detail layout.
  - Destinations own and dispose their controllers. This fixes the telemetry timer leak.
- State: `ChangeNotifier` controllers injected through constructors, matching the recording and
  attachment screens.
- Verification:
  - Flutter 3.47.8 (the CI version) was installed locally.
  - Bindings were exercised from a Dart script against the real `cdylib`.
  - No GUI run: there is no emulator, `cmake`/`ninja` are missing for `flutter build linux`,
    and no transcription engine is configured.

## 2026-10-28 — Issue #39 Meeting title CRDT sync

- Location: the issue pointed at `core/adapters/sync/`, which does not exist. The meeting
  document schema lives in `application/meeting_crdt.rs`, so the change and its README section
  are there.
- Type: a scalar string register, not Automerge `Text`. Text would merge two concurrent renames
  character by character into a title nobody typed. A register picks one whole title
  deterministically: the highest Lamport counter, then actor ID. A causally later rename always
  wins. This is not wall-clock LWW: when renames are concurrent, the winner is arbitrary but
  identical on every peer.
- Source of truth: the CRDT. SQLite `meetings.title` stays as a projection so list and detail
  reads never load CRDT bytes. Writes go CRDT first, then SQLite. A failed projection is repaired
  by the next merge or rename.
- Migration:
  - The dated fallback stays computed on read and is never written into the CRDT. A stored
    fallback would be a real register value that could beat a peer's real title.
  - SQLite-only titles from #35 are copied into documents without a `title` key when a sync
    message is generated. A document that already has a key, even an invalid one, is never
    overwritten from SQLite.
- Validation: peer values that fail `MeetingTitle::parse`, or are not strings, are ignored and
  logged without their text. They never fail the sync, because the rest of the document must
  still merge.
- Repositories returning `Unavailable` for `save_meeting_title` (test doubles) skip the
  projection instead of failing sync.
- Performance: a rename is one `put`, and an unchanged title adds nothing. Measured: about 254
  bytes for the change message and 720 bytes for a full fresh handshake on a document with 20 KB
  of notes. The test budget is 1 KiB. The issue's < 1 s propagation target depends on the network
  runtime, which is not composed yet, so it is not measured.
- Follow-ups:
  - Compose `SyncManager` in the daemon/FFI.
  - The #38 rename command should call `set_meeting_title`.

## 2026-10-28 — Issue #38 Meeting rename across CLI, FFI, and HTTP

- Storage: the issue asked for a new `update_meeting_title(&str, &str)` in the SQLite adapter.
  `NoteRepository::save_meeting_title(&MeetingId, &MeetingTitle)` already does exactly that: an
  `UPDATE` that rejects an unknown meeting, reserved by #35 as the rename primitive. No duplicate
  with weaker string types was added. A new storage test pins that it touches only the target row.
- Replication: #39 landed while this issue was in progress and made the Automerge document the
  title's source of truth. The use case therefore writes through
  `MeetingCrdtService::set_meeting_title`, not `save_meeting_title` directly. A SQLite-only rename
  would never reach peers and would be overwritten by the next merge projection. A test checks
  the CRDT title after a rename.
- Use case: `application::meeting_rename::rename_meeting` is shared by the CLI, HTTP, and FFI,
  following the `cluster_review` pattern. It validates first, so an invalid title does no I/O,
  then loads metadata to distinguish `MeetingNotFound` and to report the previous display title.
- Validation: `MeetingTitle::parse`, unchanged, enforces the issue's rules (trimmed, non-empty,
  ≤ 200 characters, no control characters, which covers newlines). Creation and rename share it.
- FFI: the issue specified a plain status code, so the function returns `c_int` rather than
  JSON. A new `FFI_MEETING_NOT_FOUND = 5` makes "unknown meeting" distinguishable from storage
  failures (`2`). Null pointers return `1` before the runtime is opened.
- HTTP: `PATCH` returns the same summary shape as `GET`/`POST`, so clients can update their state
  without another request. Error bodies reuse `{"error": "..."}`. CORS gained `PATCH`, without
  which the web dashboard could not call it.
- Paths: the issue named `core/adapters/storage/sqlite.rs` and `core/server/routes.rs`. The real
  files are `sqlite_adapter.rs` and `core/server/api.rs`.
- Follow-ups: a Dart binding and UI, web UI, and calling `notify_local_change` after a rename
  once `SyncManager` is composed.

## 2026-10-28 — Issue #40 Platform keychain for the embedding key

- Library: `keyring` 4.2 in `v1` mode, as the issue specified. It is the keyring-core rewrite,
  with one store per platform. On Linux it uses the pure-Rust zbus Secret Service client, so no
  `libdbus` headers or build scripts are needed. `keyring-core` (already transitive) is a direct
  dependency for two reasons: Windows needs a `target` modifier to get the issue's
  `note-taker:encryption-key` (the default would be `encryption-key.note-taker`), and its mock
  credential lets the adapter tests avoid the real store.
- Port, not a direct call: `KeychainStore` sits in `ports/crypto.rs`, and `KeyManager` takes it
  through `with_keychain`. This keeps the platform library out of the application layer and lets
  the migration logic be tested with an in-memory mock. `KeyManager::new` alone stays file-only,
  so existing tests and callers cannot reach a real keychain by accident.
- Key source unchanged: the database still records `key_file`. Keychain and file hold the same
  random key, so moving it needs no metadata migration, and older binaries can still open a
  database if the key is copied back to a file. Only the label changed, to "random key".
- Migration safety: the key file is deleted only after the keychain returns the identical
  32 bytes. Any store or read-back failure keeps the file and logs an error. A leftover file
  identical to the entry (an interrupted migration) is removed. A different file is never
  deleted, and both keys are tried against the key check. That case arises when a database was
  created while the keychain was locked, so an existing entry and a new key file disagree.
- Fallback levels: no store, failed initialization, and `PlatformFailure` count as "unavailable"
  and log a warning (expected on headless servers). `NoStorageAccess` and malformed entries
  count as access failures and log an error, as the issue asked. Both fall back to the file. A
  corrupt (non-32-byte) entry is a hard error, because replacing it could orphan other databases.
- No overwrite before deletion: the migrated file is unlinked, not shredded. Overwriting gives
  no guarantee on SSDs or copy-on-write file systems, and the file had the same exposure before.
- Residual race: two processes generating a brand-new key into an empty keychain at the same
  moment can each store their own key. Writes are read back and the read-back value is adopted,
  which narrows the window. There is no atomic create-if-absent in the keychain APIs.
- Build impact: zbus's `zvariant_utils` links `proc-macro2`/`quote` into the target graph. That
  gives their build-script runs the target profile and changes bindgen's hash, so
  `make rust-lint` (`--all-features`) rebuilds whisper.cpp once and needs `cmake`. The
  alternative backend (`dbus-secret-service`) would add a C library build instead.
- Verification: `make rust-lint` and `make rust-test` pass. Two `--ignored` tests (binary round
  trip, full migration) were run against GNOME Keyring under throwaway service names. A manual
  CLI run on a scratch database checked migration (byte-identical, file deleted), a restart from
  the keychain alone, and `KeyMissing`. The machine's keychain was left unchanged.
- Follow-ups:
  - macOS and Windows manual checks.
  - iOS Keychain and Android Keystore support.
  - A CLI command to export or re-key.

## 2026-10-28 — Issue #37 Unicode PDF export

- Library: stayed on printpdf 0.7. `add_external_font` already embeds TrueType fonts as
  Type0/Identity-H with a `ToUnicode` map, so switching to raw lopdf would have meant rewriting
  that by hand. The approved `font_subsetting` feature (adds `allsorts`) keeps PDFs small: the
  17 MB Noto Sans SC adds only the glyphs used. `owned_ttf_parser` 0.19 became a direct
  dependency; it was already in the tree through printpdf and is used for coverage and widths.
- Fonts (option A, approved): bundle DejaVu Sans Regular and Bold (about 1.4 MB, Bitstream Vera
  license). Discover CJK and emoji fonts at runtime instead of bundling Noto CJK, which would
  add about 20 MB.
  - Fallback order: `NOTE_TAKER_PDF_FONT_DIR`, `~/.note_taker/fonts/`, then a fixed list of
    system files. There is no recursive font-directory scan, which would be slow and
    unpredictable.
  - Only single TrueType-outline fonts are accepted. printpdf writes every font as `FontFile2`,
    so `.ttc` and CFF `.otf` would produce broken PDFs.
- Fallback granularity: per character, with consecutive characters grouped into runs. printpdf
  silently drops characters a font lacks, so fallback had to happen before drawing.
- Missing glyphs: U+FFFD (DejaVu has it) instead of `?`. It cannot be confused with real text,
  and the export still succeeds. Invisible format characters are dropped instead.
  Last-resort fallback: built-in Helvetica with the old `?` substitution, used if the bundled
  font cannot be parsed.
- `ToUnicode` fixes: printpdf writes code points above U+FFFF bare (`<1f389>`), and for shared
  glyphs keeps the lowest code point (`目` copied as `⽬`). The maps are rebuilt from the
  embedded subset fonts, preferring drawn characters, but only when a drawn character is
  affected. That avoids a lopdf reopen for typical Latin exports.
  - The bundled fonts skip the shared-glyph scan, which costs about 34 ms per face in debug
    builds. A unit test asserts they share no glyphs.
- Performance: printpdf's `write_text` clones the whole external font, including its byte
  buffer, on every call. Writing the `Tj` glyph IDs directly made Latin exports faster than the
  Helvetica path (about 5 ms against 7 ms for 300 segments, release). The test compares
  best-of-five timings against the Helvetica path with a 2x bound.
- Tests: the issue asked for system-font tests with skips. They use committed OFL subset
  fixtures (about 5 KB each) instead, so they never skip and never depend on the host. The PDF
  text extractor was rewritten to decode through `ToUnicode`, because `lopdf::extract_text`
  cannot read Identity-H fonts.
- Verification: `make rust-lint` and `make rust-test` pass. A manual export used the full
  variable Noto Sans SC and Noto Emoji: 18 pages, 103 KB. It was rasterized with poppler
  (Latin, Cyrillic, Greek, CJK, and emoji all drawn) and extracted with pypdf (text and title
  exact).
- Follow-ups:
  - Color emoji, which needs a renderer that supports `COLR`/`CBDT`.
  - Instancing variable fonts at weight 400. Google's variable Noto Sans SC defaults to Thin.
  - Checks on macOS and Windows hosts.
  - A real bold for fallback scripts.

## 2026-10-28 — Issue #41 Flutter meeting rename

- Binding location: the issue named `ui/lib/ffi/note_taker_bindings.dart`, which does not exist.
  The binding lives in `ui/lib/infrastructure/meeting_bridge.dart` with the other meeting FFI
  calls, behind the `MeetingBridge` interface, so widget tests keep using the fake.
- Errors: typed `MeetingRenameException` with a `MeetingRenameFailure` enum rather than
  `StateError`, because the dialog needs a distinct message per status. Unknown codes map to
  `unknown` instead of throwing a different type.
- Validation duplicates Rust's rules in Dart so the dialog can explain errors without a native
  call. Length counts runes (Unicode scalars), as Rust's `chars().count()` does, not grapheme
  clusters. For the same reason the field shows a custom counter instead of `maxLength`, which
  counts graphemes and would truncate input silently. Rust still validates, and status 1 is
  handled.
- Errors show after the first Save, then update live, so clearing the field to retype does not
  flash an error.
- Unchanged title: Save closes the dialog without calling Rust and shows no snack bar.
- State: the controller patches the list and the selected detail in place instead of reloading.
  It reloads only on `meetingNotFound`, so a meeting deleted elsewhere disappears.
- Accessibility: beyond the issue's long-press and right-click, Shift+F10, the context-menu key,
  and a "Rename" custom semantics action make the menu reachable without a pointer.
- Verification: a Dart script against the real `cdylib` confirmed statuses 5 and 1 through the
  isolate. A successful rename was not exercised natively, because creating a meeting requires a
  transcription engine; the Rust FFI tests from #38 cover it.

## 2026-10-28 — Issue #42 Web meeting rename

- Node.js: the issue authorized skipping local `npm install`/`npm test`, but Node 26 and the
  installed `node_modules` were available, so the tests, lint, and `next build` ran locally too.
  CI remains the authoritative check (Node 22).
- Mocks: `vi.mock` and a stubbed `fetch` (the existing pattern) instead of `msw`, which would be a
  new dependency.
- Modal: a hand-built `role="dialog"` overlay with a focus trap rather than `<dialog>.showModal()`,
  which jsdom does not implement. No UI library was added.
- List actions: both a visible "⋯" button (touch and keyboard users cannot right-click) and a
  right-click menu, since the issue allowed either.
- Errors: `MeetingRenameError.kind` mirrors Flutter's `MeetingRenameFailure`. Messages are the
  dashboard's own, because the server's text can describe internals. Axum's 422 for a malformed body
  maps to `unexpected`, and a failed `fetch` (daemon down or CORS) maps to `network`.
- Validation and unchanged titles behave as in Flutter (#41). Length counts code points
  (`Array.from`), as Rust counts `char`s.
- CI: `web-ci.yml` already ran lint, tests, and build. It now caches npm downloads, names each
  step, lists tests with the verbose reporter, and cancels superseded runs. A first version used
  `npm ci` keyed on `package-lock.json` and failed in Actions, because the lockfile is gitignored
  (`.gitignore`). It went back to `npm install`, with the cache keyed on `package.json`, whose
  versions are exact. Committing the lockfile would change project policy and was left alone.
- Docs: `API_SPECS.md` said renames were "not replicated to peers", which #38/#39 made stale; it
  was corrected.
- Verification: the bundled client against a scratch daemon returned 200 (trimmed, emoji kept),
  400 for blank, too-long, and tab titles, 404 for an unknown meeting, and `network` for a closed
  port. The CORS preflight allows `PATCH`. A headless Firefox screenshot showed the pencil sitting
  at the far edge of the page on desktop; the heading's `flex-1` was removed so it sits beside the
  title.

## 2026-10-28 — v1.0 audit and remediation

Rule applied when requirements conflicted: Security > Privacy > Reliability > Performance >
Convenience.

- **Crypto benchmark flake (db67162):** core CI on `main` was failing the 1 s embedding-encryption
  budget at 1.03–1.08 s, because debug builds run AES-GCM unoptimized. `aes`, `aes-gcm`, `ghash`,
  `polyval`, and `ctr` are compiled at `opt-level = 3` in the dev profile, and the benchmark takes the
  best of three batches. The budget itself was kept.
- **Whisper language (200b9f9):** whisper.cpp defaults to English, which would transcribe Italian
  meetings as English (4.2). `NOTE_TAKER_WHISPER_LANGUAGE` defaults to `auto` and accepts 2–3
  lowercase letters. Anything else fails at start-up, naming the variable, rather than being ignored.
- **Device ID (f90c17c):** the daemon hardcoded `headless-server`, so every server shared one
  acquisition-device ID (3.3) and one log `device_id` (6.3). Every entry point now resolves
  `NOTE_TAKER_DEVICE_ID`. Failing that, it uses `device-<uuid>`, created once with `create_new`
  in `<db>.device-id`, so concurrent first starts agree. A blank or control-character file is an
  error, not silently replaced, because replacing it would change the device's identity.
- **Log format (652dd75):** the default `tracing` JSON nested fields under `fields` and had no
  `device_id`. `StructuredJsonFormat` emits exactly the 6.3 shape. The redaction is a denylist of
  field-name fragments (embedding, passphrase, key material, token, title, transcript, PCM, …)
  rather than an allowlist: an allowlist would drop most debugging context, and a missed new field
  is still caught by a fragment such as `key` or `secret` in its name. Titles and transcripts
  count as user content and are redacted too.
- **Runtime log level for headless nodes (638a940):** the daemon has no UI, so 6.1's "CLI flags
  without restarting" is met by `cli log-level <directive>`. It writes `<db>.log-level` atomically
  (temp file plus rename), and the daemon polls it every 2 s. This uses the same control-file
  pattern as `record stop`, instead of opening an admin HTTP route, which would widen the
  network surface. A stale file from before start-up is ignored.
- **CLI log sinks (c5f9698):** the CLI defaulted to `error` so its stdout JSON would not mix with
  command output, but that also left the log file with errors only, against 6.1/6.2. A per-layer
  stdout filter now keeps stdout at `error` while the file records `info`, and `--log-level` sets
  both.
- **FFI JSON escaping (2bcd140):** hand-built JSON escaped only quotes and backslashes, so a control
  character in a title or path produced invalid JSON for Dart. Escaping now goes through
  `serde_json`.
- **Web error boundary (8e58ac7):** an unreachable daemon showed Next.js's generic error. The
  route-level `error.tsx` shows a recoverable "Try again" page and never echoes the error text,
  which could describe internals.
- **Flutter FFI off the UI isolate (f29a811, 1c94237):** recording lifecycle, attachment, and
  telemetry calls ran synchronously on the UI isolate. They read SQLite, and the first call can
  run Argon2id, which violates CONTRIBUTING.md section 9. Every bridge now uses `Isolate.run` and looks up
  symbols inside the worker. Pause, resume, and stop were added at the same time, because the
  screen could only start a recording.
- **Cross-site requests to the daemon (6c9e11a):** the API trusts loopback callers and has no user
  authentication. CORS does not cover WebSocket handshakes, and DNS rebinding defeats CORS. A
  middleware therefore refuses a present `Origin` other than the dashboard's, and a `Host` that is
  a DNS name other than `localhost`. IP literals stay allowed, because rebinding cannot produce
  them and LAN access by IP keeps working. Real per-user authentication for non-loopback
  deployments remains future work. The Teams bridge already refused browser `Origin`s.
- **Next.js security upgrade (208a030):** next 15.2.4 had 36 advisories, including an
  unauthenticated RCE. It moves to 15.5.28, the latest 15.x, which has no advisories in the GitHub
  database, instead of the breaking 16.x that `npm audit fix --force` proposes. The postcss copy
  bundled in Next still has advisories, but it runs only at build time on this repository's own
  CSS, so that risk is accepted.
- **Rust advisories (149d613):** with no `cargo-audit` available, all 537 locked crates were
  checked against the GitHub advisory API. `time` was patched with a semver update. `yamux 0.12`
  and `hickory 0.25` (libp2p mDNS) are unreachable until the swarm is composed, so a libp2p
  upgrade is now a prerequisite of #43. `lopdf 0.31` (via printpdf) parses PDFs only in tests.
- **Not fixed in the audit (too large or need approval):** P2P composition with peer pairing (#43),
  diarization (#44), Flutter microphone capture (#45), Settings with log export (#46),
  Android background controls (#47), Flutter export (#48), local screen-share capture (#49). Peer
  authorization is listed with #43 as a hard prerequisite: composing the swarm without it would
  let any LAN node that speaks the protocol read meetings and voice embeddings.

## 2026-10-28 — Approved routes for #43, #44, #45–#47

The owner decided the following after the v1.0 audit:

- **Flutter plugins (#45, #46, #47):** approved. Each issue records the plugin it picks.
- **#43 P2P sync:**
  - **Scope:** sync runs only between the owner's own devices (no colleagues).
  - **Trust model:** pairing plus a persisted allowlist. One device shows a short code; the other
    confirms it, and both compare a fingerprint of the two peer IDs. Trusted peers live in
    SQLite and are enforced with libp2p's `allow_block_list` before any sync protocol runs.
    Rejected alternatives:
    - a pre-shared `pnet` key: TCP-only, all-or-nothing, rotate-everywhere revocation;
    - owner-signed device certificates: unnecessary for one owner.
  - **Prerequisites:**
    - libp2p 0.56 → 0.57.1, which clears the yamux 0.12 and hickory 0.25 advisories;
    - a persisted node identity, since the adapter currently generates one per start.
  - **Order of work:** LAN (mDNS) first. A relay on the owner's own reachable daemon (Circuit
    Relay v2, DCUtR, AutoNAT, QUIC) follows as a separate step. Public relays are not used.
- **#44 diarization:** our own adapter on the `ort` crate (ONNX Runtime). It runs pyannote
  segmentation-3.0 for turns and SpeechBrain ECAPA (Apache-2.0) for embeddings, behind a
  `diarization` Cargo feature, with models supplied by path. The `ort` dependency is approved.
  Rejected alternatives:
  - `pyannote-rs` and `speakrs`: maintenance and maturity risk;
  - a Python sidecar: a new runtime, and no Android support;
  - cloud diarization: audio would leave the device.

## 2026-10-28 — #43 sync data model: the Automerge document is the source of truth (owner chose A)

- **Contents:** the meeting's Automerge document holds everything that syncs:
  - the title and notes, as before;
  - the origin (`created_at`, acquisition device);
  - the transcript text;
  - the transcript segments, a map keyed by `<start_ms>-<end_ms>`, with per-field values so that
    a speaker relink on one device merges with edits on another.
- **SQLite index:** the `transcript` column and the `transcript_segments` table become an index
  derived from the document.
- **Rejected:** B, append-only record replication, which would mean a second protocol to secure.
- **Mechanism:** a `MeetingDocumentCodec` port, implemented by
  `application::meeting_document`, is injected into `SqliteNoteRepository`, the same pattern as
  `EmbeddingCipher`.
  - **Local writes:** `save_transcript` and `save_transcript_segments` edit the document and the
    index in one SQLite transaction, so they can't diverge after a crash. Callers are unchanged.
  - **Every document save,** including after a remote merge, re-derives the index from the
    fields the document contains.
- **Migration:** legacy meetings whose rows predate this change are copied into their document
  once, at open time, before any projection could wipe them.
- **What stays local:**
  - unknown-speaker clusters, chunk embeddings, and review clips (biometric review data on the
    recording device);
  - attachments' file bytes (`FileSyncManager`).
- **Voice library:** replicated separately later. Embeddings must never sit unsealed in a
  document BLOB.

## Encryption key storage: no silent key-file fallback (2026-10-28)

The owner decided:

- **No silent fallback.** When the keychain is enabled and can't be reached, or refuses to store
  or read the key, the core no longer falls back to a key file:
  - the daemon and the CLI exit with an error;
  - the Flutter app shows the error with Retry.
- **Explicit opt-out stays.** `NOTE_TAKER_KEYCHAIN_ENABLED=false` without a passphrase still
  uses a key file, but only because someone chose it.
- **Migration stays.** An existing key file is still migrated into a working keychain and then
  deleted.
- **Android uses the Android Keystore.** The random key is wrapped by a non-exportable,
  hardware-backed Keystore AES-GCM key, through a small Kotlin bridge with Android SDK APIs only
  (no new library). If the Keystore fails, the app shows the error with Retry, not a key file.

## 2026-10-28 — Android key hand-over over JNI; app lock

- The owner approved the `jni` crate. Kotlin now passes the Keystore-unwrapped database key to
  Rust through `NativeCore.provideEncryptionKey` (`core/bin/ffi_android.rs`). Before, the key
  crossed a Flutter platform channel, whose read-only buffer left an unwipeable copy in the Dart
  heap. `jni` 0.22.4 was already in the Android build through hickory's mDNS, so no new code
  ships. It is an Android-only dependency.
- The owner asked for an optional banking-style app lock on Android, toggled in Settings. It is a
  UI gate over the framework `BiometricPrompt` (strong biometrics or the device credential, no
  AndroidX library). It locks at start-up and after 30 s out of sight, and it fails closed if
  the setting cannot be read. Changing the setting needs a successful prompt. The recent-apps
  preview is hidden while the lock is on.
- Not chosen: binding the Keystore wrapping key to user authentication. That would make the
  lock cryptographic at a cold start, but Android permanently invalidates such keys when the
  screen lock is removed (and, for biometric keys, when a fingerprint is enrolled), which would
  make the local database unreadable. A later background start without UI (#47) could not
  unwrap it either.
- The lock covers only the UI, so recording and sync are not interrupted. Future
  notification and lock-screen recording controls (#47) need no app unlock.

## 2026-10-28 — #45 microphone capture: `cpal` in the Rust core (owner chose option 1)

- Capture happens in Rust with `cpal` 0.18 behind a `microphone` Cargo feature:
  - ALSA on Linux (through PipeWire/PulseAudio's ALSA plugin);
  - WASAPI on Windows;
  - AAudio on Android.

  Not chosen: the Flutter `record` plugin. On Linux it shells out to `parecord`, `pactl`, and `ffmpeg`, its audio crosses Dart isolates, and the CLI and daemon would gain nothing.
- The default input device opens in its own format. `PcmResampler` (in-house) downmixes it and
  low-pass filters it (windowed sinc, cutoff 7.2 kHz). Linear interpolation then brings it to the
  16 kHz mono PCM16 the recorder expects. No resampling crate was added.
- A dedicated thread owns each `cpal` stream, because streams are not `Send` on every platform.
  Frames reach the recorder through a channel.
- FFI:
  - start opens the microphone first (status 7 if it cannot) and then the recording;
  - pause, interruption, and stop close it and push the tail, so stop flushes the last audio;
  - a ticker thread drains frames under the runtime lock, so stop (which holds that lock while
    transcribing) never waits on a thread that waits on the lock.
- CLI: `record start` without `--input` records from the microphone.
- Android asks for `RECORD_AUDIO` through our own Kotlin channel (no permission plugin).
- Trade-off accepted: no Android hardware echo cancellation or noise suppression.

## 2026-10-28 — Ship the Whisper model; #47 background recording with our own service

**Model.** The owner asked for an app that works out of the box. The multilingual `ggml-base.bin`
(148 MB) now ships in the Linux and Windows bundles (`data/models`) and as an uncompressed APK
asset that `WhisperModel.kt` copies into the data directory on first start (whisper.cpp needs a
file path). A model placed in the data directory still wins. The APK grows to about 218 MB:
fine for sideloading, but Play Store distribution would need an asset pack, and a quantized model
(`ggml-base-q5_1`, about 57 MB) is the fallback if size matters more than accuracy.

**#47.** `RecordingService.kt` is our own foreground service of type `microphone` (no plugin, as
planned on the issue). Dart mirrors its state into the service; the notification's Pause, Resume,
and Stop call the Rust lifecycle functions over JNI (`ffi_android.rs`), so they work with the app
in the background or the screen locked, and the service reports each change back to Dart. The
lock-screen version of the notification hides the meeting title. POST_NOTIFICATIONS is asked
together with RECORD_AUDIO on Android 13 and later. Desktops need nothing: capture is native and
continues while the window is minimized.


## 2026-10-28 — #46 Settings: log level and log export on every platform

- **The app logs from start-up.** It used to start file logging only when Dart set a level, which
  it never did. `ffi_logging.rs` now starts it when the runtime opens, before the stores, in
  `<data>/logs`. The logging runtime sits in a process-wide cell, because the subscriber can be
  installed once while opening the runtime can be retried (a locked keychain).
- **Persistence differs by process, on purpose.** The app stores the Settings level in
  `<db>.log-level` and reads it back at the next start, as owners expect of a setting. The daemon
  keeps its rule: `RUST_LOG` sets the start level, and changes from `cli log-level` or the web page
  last until it restarts. Both share `LogLevelControlFile`'s validation.
- **One export format.** The newest seven daily files, joined oldest first into one JSON-lines
  file (`export_log_files`), bounded so a long-lived install shares a reasonable file. It is
  served by FFI (to a path) and by `GET /api/logs/export` (as a download). Entries were redacted
  when written.
- **Android sharing without a library.** Our own read-only, non-exported `SharedFileProvider`
  serves only `cache/shared`, where the export is written; the share intent grants the chosen app
  read access to that one file. `share_plus` and androidx `FileProvider` were not added.
  Desktops reuse `file_picker`'s save dialog.
- **Web.** `/settings` changes the daemon's level and downloads the logs. The routes depend on a
  `LogControl` port, so they are tested without the global subscriber; the daemon's control-file
  watcher applies `cli log-level` through the same `LiveLogControl`, so the page shows the level
  in effect.

## 2026-10-28 — #53 Telemetry: recorded by the pipeline, shown under Settings

- **Recorded where the work happens.** `MeetingSession` times transcription and diarization per
  chunk and saves a row through an optional `TaskMetricsRecorder` (`MetricsRepository` port).
  `ComputeRouter` stays as it is; routing live recordings through it would be a larger change
  than the empty dashboard calls for. Failed tasks record nothing. A failed telemetry save is
  logged and never fails the chunk.
- **Labels are privacy-safe.** A local model is labelled by its file stem (`ggml-base`), never its
  path, which can name the user and is visible to peers.
- **Energy is an estimate.** Local whisper.cpp is CPU work at 5 W, the router's existing CPU
  figure. Cloud transcription is 1 W, since the device mostly waits on the network. Neither is
  measured.
- **Placement (owner's request).** Telemetry is a page of Settings in the app and
  `/settings/telemetry` on the web; `/dashboard` redirects there. The web home's duplicate
  aggregate card was removed.

## 2026-10-28 — #51/#52 Chunking: no overlap, short chunks, no prompt carry-over

Each choice below was measured with the real ggml-base model: jfk.wav ×3 at four cut positions,
and a 90 s Italian clip.
- **No overlap.** A 1 s lookback was transcribed twice, and nothing could reliably remove the
  repeated words. Text de-duplication would also delete genuine repetitions. A forced cut now
  lands at the end of the quietest 30 ms frame in the last 3 s, so every sample is transcribed
  exactly once.
- **15 s at most.** This bounds how far the live transcript lags, in both modes (was 120 s with
  VAD and 30 s without). With VAD, a pause ends a chunk only once it holds 3 s, which avoids
  fragments Whisper handles poorly.
- **Silent chunks are skipped.** Whisper invents text for silence. The timeline still advances.
  Marker-only segments (`[BLANK_AUDIO]`) are dropped as well.
- **No previous-text prompt.** With it, Whisper echoed the previous chunk's last phrase (2 of 4
  runs) or garbled the boundary (2 of 4). Without it, all 4 runs were exact. The configured
  language keeps chunks consistent.
- **Live transcript by polling.** The app polls the active meeting's segments every 2 s
  (existing FFI) instead of adding a push channel. Chunks arrive every few seconds, so polling
  is enough and adds no new FFI surface.

## 2026-10-28 — #49 Screen-share slides: per-app detection, grid-diff heuristic

The owner asked for the harness only, with no new app supported yet. The full research is in
the comment on #49.
- **Detection is a port, per conferencing app.**
  - Windows: Graphics Capture can't be queried for other processes' captures.
  - Wayland: PipeWire screencast nodes don't reliably name the app or monitor.
  - X11: no signal at all.
  - macOS: no supported API.

  So `ScreenShareDetector` is for an app adapter to implement, and there is no OS-level guess.
  Until one exists, `NoScreenShareDetector` means nothing is ever captured.
- **New-slide heuristic: block-based frame differencing, as in lecture-video slide extraction.**
  - Luma is area-averaged into a 32×18 grid. Frames differ when at least 6 cells move by at
    least 8 levels.
  - A frame is saved once it is stable and differs from the last saved slide.
  - Per-pixel differences are too noisy, and a 64-bit dHash misses a new bullet line.
  - The constants are starting points, to tune on real shares.
- **A watcher thread per recording, sampling every 2 s.**
  - Frame capture stays off the audio path.
  - It runs only while audio is captured, so pausing also pauses screen capture.
  - Slides carry the meeting time, so they sit at the right point of the transcript.
- **Slides reuse `SlideAttachmentStore`.** The store keeps the Teams files' layout and
  permissions and rejects meeting IDs that are not plain folder names. Slide files are not
  encrypted, the same policy as other attachments.

## 2026-10-28 — #54 Photos and attachments: `camera` plugin, private copies, meeting time

- **Camera: the `camera` plugin plus `camera_windows`**, approved by the owner. It covers
  Android, iOS and Windows; Linux has no implementation, so Linux attaches files only. The app
  opens the back camera without audio and attaches each shot at once, deleting the plugin's
  temporary file whether or not the attach succeeds.
- **Attachments are copied, not referenced.** Picked files may be cache copies the system
  clears (Android) or temporary camera files, so `AttachmentFileStore` copies each into
  `<data>/attachments/<meeting>/<uuid>-<sanitized name>` (0700/0600 on Unix). Only pictures and
  common document types up to 100 MiB are accepted. They are not encrypted at rest, the same
  policy as slides; notes and voice embeddings remain the encrypted data.
- **Placement by meeting time.** An attachment taken while recording carries the recording's
  meeting time (`attachments.meeting_time_ms`) and is shown before the first transcript segment
  starting after it; one added afterwards has none and goes at the end. Slides use the same field.
- **One flow replaces the old import screen.** "Take photos" and "Attach files" (multi-select)
  are on the Record screen and the meeting detail; the single-file import screen was removed.

## 2026-10-29 — #43 wrap-up: CLI revocation through the database; manifests on the sync stream

- **The CLI edits the allowlist directly; the running task reconciles.** The CLI has no HTTP
  client and adding one would be a new library, so `devices revoke` deletes the row in the shared
  database. The P2P task compares its gate with the stored allowlist on its existing one-second
  tick (`PairingManager::reconcile_trusted_peers`) and drops and disconnects removed devices. The
  stored allowlist stays the single source of truth. Pairing stays on the Devices screens, since it
  needs the code comparison on a running device.
- **Manifests share the encrypted sync stream, with their own magic.** `NTM1` + JSON beside
  `NTS1` sync envelopes avoids a new libp2p protocol; older devices reject the unknown envelope
  and carry on. A manifest is sent when a paired device connects, accepted only when it names its
  sender, bounded in size and content, and forgotten on disconnect or unpairing, so the status only
  lists devices that could take work now.
- **Advertised models come from the pipeline configuration, not from loading the model.** Loading
  Whisper just to advertise it would cost seconds at start; a model file that later fails to load
  is still advertised (the pipeline reports that failure itself). GPU memory is advertised as 0
  until a detection adapter exists.
- **The web Devices page mirrors the Flutter screen** in steps and wording, polling the status
  every two seconds; server error text is never shown.

## 2026-10-29 — #50 unknown-speaker clusters across paired devices

- **The owner chose to share clusters, embeddings and review clips** with paired devices, with
  the raw audio clips under an extra encryption layer on top of the Noise transport. Through the
  follow-up questions the owner picked a per-pair key and encryption of clip audio at rest.
- **Review-clip audio is sealed at rest** with the database cipher, like embeddings, under
  `unassigned_voice_clips:audio:<clip id>` and an `audio_format` column; older plaintext rows
  are sealed once when the adapter opens.
- **Pair keys come from static X25519 secrets.** Each device keeps one secret, sealed in the peer
  store, and sends its public key (`NTK1`) to each connected paired device; HKDF-SHA256 over the
  shared value, bound to both peer IDs and public keys, gives the pair key. A static secret
  needs no coordination between the two devices, survives reconnection and re-pairing, and
  covers devices paired before #50, which agree at their next connection. Ephemeral keys would
  add forward secrecy, but the Noise transport underneath already provides it for traffic. The
  first key is pinned in the allowlist row and a different one is ignored, so a later message
  cannot swap it. The owner approved `x25519-dalek` (3.0, already built by libp2p-noise) and
  `hkdf` (0.12, matching the direct `sha2` 0.10).
- **Clusters travel as sealed shares, not in the meeting document.** Clusters hold embeddings
  and raw audio, which must be deletable; an Automerge document keeps its history forever. So
  `NTC1` shares carry them, sealed with the pair key, and each device re-seals them at rest with
  its own key. Each device is sent only what it lacks, and a share whose meeting has not synced
  yet is held briefly in memory.
- **Resolutions travel in the meeting document.** Naming a cluster writes
  `cluster_resolutions/<cluster> = <identity>` into the document; projecting it, on any device,
  resolves the cluster, deletes its clips, and sets the speaker of the cluster's segments. One
  register per cluster makes concurrent namings converge on Automerge's winner everywhere, and
  a share arriving after the resolution stores nothing, so a named speaker's audio never comes
  back. The identity's name and voice variants stay in the device that named it until the voice
  library syncs (#56); until then other devices show those segments as an unknown speaker, and on
  a concurrent naming the losing device keeps the variants it enrolled under its own identity.

## 2026-10-30 — #44 diarization: runtime, models, and thresholds

- **ONNX Runtime is downloaded at build time** (`ort` `=2.0.0-rc.13`, its `download-binaries`), the
  owner's choice. rc.13 is the last release within our Rust 1.88 MSRV; rc.14 needs 1.92.
- **Models come from a Python export script in the repo** (`tools/diarization/export_models.py`),
  the owner's choice. Python runs only at export time; the app never needs it. The filterbank is
  inside the ECAPA graph, so no feature extraction had to be written in Rust.
- **Requested without models means unattributed, not a failure**: transcription keeps working on a
  device without models. Named models that cannot load are an error, because that is a mistake
  to fix rather than a state to run in.
- **Thresholds, measured with the exported ECAPA on Mini LibriSpeech dev-clean-2** (cosine,
  different utterances, 26 speakers):

  | Segment | Same speaker: 5th / 50th percentile | Different: 99th percentile |
  | --- | --- | --- |
  | 1 s | 0.09 / 0.30 | 0.27 |
  | 2 s | 0.29 / 0.50 | 0.31 |
  | 4 s | 0.49 / 0.68 | 0.33 |
  | 8 s | 0.58 / 0.80 | 0.32 |

  The clustering threshold moved from 0.70 to **0.40**, since at 0.70 only 41% of same-speaker
  4 s pairs would join. The window link threshold is **0.35**; turn error was flat from 0.2 to 0.4
  on 40 spliced 15 s chunks.
- **The matcher's 0.80/0.50 cutoffs are unchanged.** At 0.80 only long audio reaches a strong
  match, so most recognized voices will ask for confirmation. That errs on the safe side, and
  retuning needs measurements with real enrolled voices; it is left as a policy decision.
- **Measured quality is limited**: spliced clean recordings, not real meetings with overlap, noise
  or far-field microphones. On the 40 chunks, mean speaker confusion was 3.5% (median 0), and 4
  chunks merged both speakers into one in segmentation. Short turns give weak embeddings.

## 2026-10-30 — #55 relay through the owner's daemon

- **Circuit Relay v2 plus DCUtR and Identify.** AutoNAT and QUIC, also named in the approved route,
  are left out: the relay does not need to detect reachability, because its address is configured,
  and hole punching works over TCP. They can follow if TCP punching proves too weak.
- **Access is the existing allowlist.** The gate already refuses unpaired peers. The pairing window
  is the one gap, closed by disconnecting an untrusted peer as soon as it uses the relay, since
  libp2p's relay has no per-peer filter. The stranger could never get further than the destination's
  own gate.
- **The relay's address is configured, not detected**, because reservations need an address to hand
  out and an always-on host knows its own.
- **The daemon stays a normal paired device.** Relayed traffic is ciphertext to it, but pairing it
  also means it syncs and stores the owner's meetings, like any other paired device. A relay-only
  mode was not built: the issue names the owner's own daemon, which is meant to hold them.
- **The apps take the relay address at build time** (`--dart-define`), like static peers, until
  Settings has a field for it.

## 2026-10-30 — #56 voice-library sync

- **Sealed messages, not the Automerge document.** Embeddings must be deletable, and a document keeps
  its history forever. `NTV1` messages carry them under the pair key, and each device reseals them
  with its own key, as with cluster shares (#50).
- **Variants union by ID; names follow the newest `updated_at`**, with the greater name winning at
  the same instant, so every device converges in any order. It relies on device clocks: a device with
  a wrong clock can win or lose a concurrent rename, which is harmless compared with coordination.
- **Deletions always win and their records are kept forever.** Deleting a voice is a privacy promise,
  so a stale device must not restore it, whatever its timestamps say. Records hold only a UUID and a
  time. The cost is that a deleted UUID can never be used again; new enrollments get new UUIDs.
- **The whole library is sent when its digest changes**, rather than per-variant deltas: a personal
  library is small, and it needs no per-device state to survive restarts.
- **Merging two people is not built**, despite the issue's wording: no device has such an operation
  to sync. Concurrent namings of one cluster keep the behaviour from #50: both identities sync, and the
  document's winner is the name segments show.

## 2026-10-30 — #57 attachment sync

- **The receiver pulls one chunk at a time.** The first `FileSyncManager` pushed a whole file at
  once and held it in memory; it was never wired in. With up to 1,600 queued 64 KiB requests on a
  transport with a 10 s request timeout, large files could fail on slow links. Pulling bounds memory
  to one chunk, and a lost chunk is simply asked for again. It costs one round trip per chunk. The
  manager, its content-addressed store and the port's availability broadcast were removed.
- **Metadata goes in sealed `NTA1` lists, not the Automerge document.** Attachments never change and
  have no deletion yet, so a list needs no merge rules, and the document stays free of local files'
  metadata. Lists are sent on connection and when they change.
- **A row exists only with its verified file**, so the app has no "file still missing" state to show.
  The cost is that an attachment appears only when its file has arrived.
- **Interrupted transfers restart from the beginning.** Files are at most 100 MiB and nothing about
  a transfer needs persisting; a device fetches what it lacks whenever a list arrives.
- **Existing attachments are hashed lazily**, one per tick on the P2P thread (a short pause even for the
  100 MiB maximum), and the hash is kept in the row, instead of a blocking migration at start-up.
- **Files stay unencrypted at rest**, private to the user, as `AttachmentFileStore` already keeps
  them locally. The issue asks for the same treatment; encrypting attachments would be a separate
  change on every device.
- **Unlinked attachments stay on their device** until the owner links them to a meeting.

## 2026-10-30 — #59 transcription on a paired device

- **The choice starts as Local.** Raw audio is the most sensitive thing the app holds, so it leaves
  a device only after the owner picks another option. Auto is opt-in too.
- **Sealed messages over the sync channel, not a new protocol** (`NTT1`/`NTT2`, like cluster
  shares). Audio is under the pair key as well as Noise, and a chunk fits the 16 MiB payload limit;
  a longer one is refused and transcribed locally.
- **Every remote failure falls back to local**, without retrying another device. The chunk is only
  copied for the attempt and deleted after success. A device with no engine reports the chunk as
  failed, and the audio is deleted at stop like any untranscribed chunk.
- **A dispatcher is built per chunk** from the latest manifests, rather than kept in step with
  connection events: it stays correct by construction, and it costs a few allocations per chunk.
- **Hosting has a queue of 4 and a lazily built engine.** A device nobody asks loads no second
  model; a device that hosts while recording holds two. Past the queue, a device is told to fall
  back at once.
- **Known limits.** An answer waits for the host's next 1 s tick. Teams bridge audio stays local.
  Telemetry records a remote chunk's time against this device.

## 2026-10-30 — #60 diarization in builds (partial)

- **Enabled only on Linux**, the one platform where `ort` has been run. `x86_64-pc-windows-gnu` has
  no prebuilt ONNX Runtime; the Android library links but has not been run on a device.
- **Models in `<data directory>/models/diarization/` turn the feature on**, so no setting is needed
  once a device has them. How they get there is still the owner's decision (see #60).

## 2026-10-30 — #60 diarization models in the installers

Owner's decisions: ship the models in the installer; Windows loads `onnxruntime.dll` at run time;
measuring and retuning moves to #61.

- **Bundled models live beside the executable** (`data/models/diarization`), passed to the core as
  `diarization_model_directory`. A pair in the data folder still wins, as with Whisper.
- **Windows uses `ort`'s `load-dynamic`** through target-specific dependencies, so other platforms
  keep the linked prebuilt. `load_session` calls `ort::init_from` first, because `ort` panics on
  first use if the library cannot load; a failed load now means "diarization unavailable".
- **ONNX Runtime 1.28.0 on Windows**, the version `ort` builds against elsewhere, from Microsoft's
  release with a recorded SHA-256. Windows 11 has its own 1.17 in System32, which `ort` refuses as
  too old, so the DLL beside the executable is the one that must load.
- **The Windows bundle ships the Visual C++ runtime** (`InstallRequiredSystemLibraries`): a clean
  Windows 11 does not have it, and `onnxruntime.dll` fails to load without it.
- **Android ships it too, after a run on a Galaxy S23.** A real recording produced separate unknown
  speakers. Models are uncompressed APK assets copied to the data directory on first start. The APK
  grows from 221 MB to 332 MB. Mobile quantization was not needed to run; #61 can revisit it.
- **Export is not pinned by hash.** `fetch.sh` exports the models, and export may not be
  byte-reproducible; the export script validates them with onnxruntime instead.

## 2026-10-30 — #62 choosing the transcription language

- **The language is stamped on each chunk when it is cut.** Phones can fall behind a live recording,
  so reading it at transcription time would apply a change to audio spoken before it.
- **The list is whisper.cpp's own 100 codes plus `auto`.** whisper.cpp fails every chunk on an unknown
  code, so the core refuses anything else, including in messages from paired devices.
- **The default lives in `<db>.transcription-language`**, like the log level: not secret, and readable
  without the database. Order: stored choice, `NOTE_TAKER_WHISPER_LANGUAGE`, AUTO.
- **A recording's language ends with it.** The next recording starts on the default, and changing the
  default during a recording does not change that recording.
- **Requests to paired devices are now `NTT3`**, because they carry the language. A device on an older
  build ignores them, the request times out, and this device transcribes locally.
- **Apps only.** The CLI and daemon keep `NOTE_TAKER_WHISPER_LANGUAGE`, and the web dashboard has no
  recording controls to add a selector to.

## 2026-10-30 — #63 the voice library has its own destination

- **Voices sits in the main navigation**, between Meetings and Devices, so naming a voice no longer
  starts from one meeting. Unknown voices from every meeting and all saved people appear together.
- **Review clips are written only when Play is pressed.** Listing the library decrypts no audio, and
  clip deletion keeps working as before. The web dashboard reads the same clip through
  `GET /api/clusters/{id}/review-clip`.
- **Raw embeddings never leave the core.** The new views carry counts and labels only.

## 2026-10-30 — #64 relay and device addresses at run time

- **Settings applies addresses live**, with no restart. A release build needs no compile-time define.
- **Saved values replace the environment and defines entirely**, rather than merging, so what Settings
  shows is what the device uses. They are stored in SQLite (`p2p_network_addresses`).
- **Removing the relay does not affect the local network.** Checked with a restart on the VMs.

## 2026-10-30 — #65 pairing invites

- **Format:** `note-taker-invite:1;<peer>;<expires_ms>;<addr>,<addr>`. It holds no secret and is treated
  as untrusted input.
- **An invite only finds a device.** It puts the peer under Nearby, and the owner still compares the
  code on both sides. It grants no trust.
- **It lasts for the 5-minute pairing window**, and may end sooner when a pairing closes the window. It
  is reusable until then.
- **Invited addresses are kept after pairing and removed on revoke**, so the pair can reconnect.
- **A relay refuses strangers**, so using a circuit address requires the joining device to be paired
  with that relay first. The relay remains a device the owner controls.
- **`qr_flutter` is the only new dependency** (approved by the owner). The apps display a QR code but do
  not scan one, since a scanner would need a further dependency: a camera app copies the link. The web
  dashboard shows the link only, because the approved library is Flutter only.

## 2026-10-30 — #66 explaining Nearby and disconnected devices

- **Nearby states that it covers this network only**, and points to invites, and to relay settings when
  no relay is configured. A disconnected device says what it needs in order to reconnect.
- **A device answers a peer's first manifest with its own.** Otherwise the side still waiting for its
  owner to confirm drops the other's manifest, and shows a connected device as "Not connected".

## 2026-10-30 — #61 first measurement on real voices

Fifteen readings of `tools/diarization/test_scripts` by three people, 72 to 95 s each, through the real
models with fixed 15 s chunks and `ggml-base`. The reference comes from aligning transcripts with the
scripts, so figures are approximate and 1.5 s around each change of speaker is not scored. Each of the
three solo recordings was enrolled as one voice.

- **The right number of speakers in 9 of 15 recordings**, with 8.3% of speech missed and 11.3% given to
  the wrong speaker at the current cluster threshold (0.40). From 0.40 to 0.50 the results are flat, so
  it stays.
- **Merged chunks are the largest source of error.** 28 of 87 chunks found fewer speakers than spoke, and
  each speaker in a chunk gets one averaged embedding, so a merge mixes voices. Turns in chunks with the
  right speaker count pick the right person 93% of the time, against 69% in the others.
- **Raising `speaker_link_threshold` does not fix it.** From 0.35 to 0.65, merged chunks fall only from
  28 to 21, split chunks rise from 4 to 9, and matching does not improve. It stays at 0.35.
- **The matcher's cutoffs were not changed.** At the strong cutoff of 0.80 only 12% of a person's turns
  match automatically, with 1% above it for the wrong person. Lowering it to 0.70 gives 44% but 8%
  wrong, and a wrong automatic match is silent. The 0.50 suggestion cutoff catches 85%, with 18% wrong.
  Reassess after merges are reduced.
- **Not covered:** noise, far-field microphones, overlapping speech, other voices. One session, one
  microphone and three people cannot settle the cutoffs, so #61 stays open.

## 2026-10-31 — What limits speech-to-text and voice recognition

Same 15 recordings, about 21 minutes in all, one session and three people, with a speaker change about
every 5 s. Tools: `tools/diarization/evaluate_pipeline.py` and `evaluate_short_windows.py`. No product code
changed; the design below awaits the owner's decision.

- **The Whisper model is the largest transcription limit.** Word error on chunks: `base` 29.6%, `small`
  17.0%, `medium` 7.9%, `large-v3-turbo` 5.5%. Auto language and chunking cost little here.
- **Voices were not the problem.** All 228 cut-out turns with a voice measurement matched the expected
  reader's solo recording, Ettore and Renzo included.
- **The 10 s windows were.** Today's confusion is 11.4% and whole-meeting `pyannote` gives no improvement
  overall (error 16.7% against 15.3%). Embeddings from 2 s windows with one clustering per meeting give 1.8%
  confusion, 8.8% error and the right speaker count in 14 of 15. Choosing the threshold on this same data
  flatters this, and error rises sharply below 0.2.
- **Word timing and per-speaker naming are worth adopting.** With known speaker times, word timing gives
  98.2% against 91.5%. Per-speaker naming gives 95% right against 77% per turn at cutoff 0.60.
- **Naming a merged cluster damages the gallery.** At the 0.80 cutoff, wrong automatic names go from 0.3% to
  7.3% when enrolling from a named cluster.
- **All inference runs on the CPU.** `use_gpu` is hard-coded off, ONNX Runtime has no other provider, and
  the Android build has only baseline ARMv8. Vulkan runs turbo about 20 times faster on an RTX 2000 Ada and
  3.8 times faster on Intel Arc, with the same word error. The S23 is not measured.
- **Silero through whisper-rs does nothing.** `WhisperState::full` bypasses whisper.cpp's VAD, so the option
  is silently ignored and WebRTC against Silero remains untested.
- **Not covered:** noise, distant microphones, overlapping speech, in-app capture, phone speed, and a Rust
  build of the candidate design.

## 2026-10-31 — #67 choosing the Whisper model and OpenAI in Settings

- **The core owns it.** Choices live in `<db>.transcription-settings.json`, and Flutter only displays
  them. A saved value replaces the environment's; the daemon and CLI keep using the environment.
- **Changes apply when the next recording starts.** The idle pipeline is rebuilt then, so a running or
  paused recording never loses audio. The device manifest and paired-device hosting read the choices at
  app start, so those need a restart.
- **Downloads are verified before use.** Bytes go to `<file>.part`, and only a file matching its pinned
  size and SHA-256 gets its final name. All 33 files from Hugging Face are offered, except `small.en-tdrz`.
- **A saved key alone selects Local.** Only an explicit choice, after a confirmation, sends audio to OpenAI.
- **Android has no keychain the core can reach, so its key is in a file sealed with AES-256-GCM** under the
  database key that the Android Keystore protects. This differs from the issue's wording: refusing would
  have left Android with no OpenAI option, and the file is useless without the device's Keystore. Desktops
  use the OS keychain under its own entry, and refuse when there is none.
- **OpenAI failure falls back per chunk.** A connection failure, refusal or rate limit transcribes that chunk
  with the local model, which loads only when first needed. Audio is never queued for later upload.
- **Not on the web dashboard.** It has no recording controls (#62), and a server's key belongs in its
  environment, so the API has no route that could expose a key.
- **`libc` became a direct dependency** for free disk space and total memory. It was already in the build.

## 2026-11-01 — #70 CPU instructions and threads, #71 desktop GPU

- **Vulkan on Linux and Windows, with the CPU as fallback.** Same clip on an RTX 2000 Ada against the CPU, as
  multiples of audio length: `base` 0.009 against 0.072, `small` 0.019 against 0.245, `large-v3-turbo` 0.039
  against 1.25. The first run after install took 0.51 for `base`, because the driver compiles its shaders once.
- **A discrete GPU beats an integrated one**, even when listed second. We advertise only a discrete GPU's memory
  to routing: an integrated GPU's figure is shared system memory (50 GB here) and would outrank a real GPU.
- **No build-time link to Vulkan.** A normal link stops the app starting where the Vulkan loader is missing. The
  core defines the three functions ggml calls directly, and opens the loader on first use
  (`vulkan_loader_shim.rs`). Checked on `deb-shell` (no loader), `deb-gui` (loader, software renderer only) and
  `win11` (with and without `vulkan-1.dll`): each transcribes on the CPU. A real GPU on Windows is untested.
- **A model that won't fit stays on the CPU.** It needs 1.5 times its size plus 256 MB free on the GPU, and a
  trial allocation at load falls back too. `NOTE_TAKER_WHISPER_GPU=off` forces the CPU.
- **At most 8 threads, fast cores only.** `small` on a 22-thread Core Ultra 7, CPU: 4 threads 0.32, 8 threads 0.24,
  16 threads 0.22, 20 threads 0.39, 22 threads (the old setting) 0.33. Above the fast cores it gets worse, and 8
  is close to the best while leaving the machine usable. A core counts as fast at 70% of the top speed.
- **Fixed minimum CPUs.** Linux no longer uses `-march=native`: like Windows it needs AVX2, FMA and F16C. Android
  needs `armv8.2-a+dotprod+fp16` (Cortex-A55 and A75, 2018, on). Without these the engine reports itself unavailable
  instead of crashing. The ARM build is compiled but **not run: S23 figures for #70 still need the phone**.

### S23 figures for #70 (Snapdragon 8 Gen 2, 8 GB; 11 s clip, multiples of audio length, third pass)

| Model | Old build, 8 threads | New build, 8 threads | New build, 5 fast cores | Peak memory |
| --- | --- | --- | --- | --- |
| `base` | 0.35 | 0.14 | **0.14** | 0.33 GB |
| `small` | 1.21 | 0.53 | **0.45** | 0.77 GB |
| `medium` | 4.48 | 3.42 | **1.44** | 1.95 GB |
| `large-v3-turbo` | 8.53 | 7.05 | **2.88** | 1.95 GB |

- **`small` is now the largest model an S23 can run live**: 0.45, against 1.21 before. `medium` and turbo stay slower
  than real time. These runs got warmer as they went (28 to 38 °C), so the later rows may include some throttling.
- `base` barely depends on the thread count from 4 up (3: 0.18, 4: 0.15, 5: 0.14, 8: 0.14); the gap widens with model size.
- Not checked: that the installed app can read core speeds as `adb shell` can. Our release app can't be inspected, and
  if it can't read them it uses 8 threads, as in the middle column.

## 2026-11-01 — #73 whole-meeting speakers, #74 naming once and saving clean voices

- **Two passes.** Live chunks and labels stay as they were. During a recording the session also keeps one voice
  embedding per 2 s window, every second, in memory and without audio. When the recording stops, it groups them all
  (average linkage), reassigns every word to the nearest window's speaker, and replaces the live segments. After a
  crash the live result stands, and the embeddings are gone with the process.
- **Measured through the Rust code on the 15 recordings** (`evaluate_meeting_pass.py`):

  | Measure | Live design | Final pass, threshold 0.2 | Target |
  | --- | --- | --- | --- |
  | Speaker error | 15.3% | **5.3%** (missed 0, false alarm 3.1, confusion 2.2) | at most 10% |
  | Right speaker count | 11/15 | **15/15** | |
  | Words under the right speaker | about 85% | **94.4%** turbo, **95.0%** `base` | at least 95% |
  | Wrongly named speech, 12 multi-speaker files | about 5% per speaker | **0%** | at most 1% |

- **The 95% gap on turbo is explained.** Words over 1 s from a turn change are 99.4% right, and words within 1 s are
  86.2%. These scripts change speaker about every 5 s, far more often than a normal meeting, and a window is 2 s.
- **Threshold 0.2, tuned on these same recordings, so the figures flatter it.** 0.19 and 0.2 find all speaker counts,
  0.21 to 0.3 miss one or two with error 6.2–6.8%, 0.18 gives 7.0%, and 0.15 gives 13.2%. Final tuning needs the
  natural recordings (#72).
- **Missed speech comes from quiet speech.** The windows left without a voice are quieter than the rest: median level
  0.014 against 0.041 for Ettore, and Marta, the quietest reader (0.021), loses most, up to 32.5% in one file.
  All of it was inside VAD chunks. It no longer loses words or speaker time, because each word and moment takes the nearest
  window in its chunk. The price is the 3.1% false alarm, where a chunk's pauses count as speech.
- **Each speaker is named once, at 0.60**, from the average of all their speech: right matches reached about 0.66 and above, and
  the one wrong nearest match at threshold 0.25 was in a mixed group. Below that they stay unknown. If two speakers match
  one person, only the closer keeps the name. A name the owner gave during the recording is kept.
- **Naming a group saves one averaged voice, not each member.** It needs 8 s of speech and members averaging 0.60
  similarity to their centre. The final pass marks a group under 20 windows or under that consistency as not saveable.
  Naming still works without a saved voice; the app just doesn't learn it from that group.
- **Not covered.** Cost on a phone: 0.064 times audio length on a desktop CPU, and the S23 disconnected before it
  could be measured. A full recording through the app with a microphone. Whether later matches improve over time, since
  this test enrols from solo recordings. Confirming a single weak-match clip still saves that clip's voice, by the
  owner's explicit choice. A final speaker with no live cluster has no review clip, because no audio is kept for it.

## 2026-11-01 — phone GPU trial (#75), voice-window cost, and a VM with a GPU

- **Phones stay on the CPU.** A Vulkan build of whisper.cpp on the S23's Adreno 740 aborts during model setup, with an
  uncaught C++ exception, for both `small` and `large-v3-turbo`. No speed or battery figures exist as a result. `base` wasn't
  tried, because USB dropped during the pushes, and the exception's text wasn't captured. Shipped Android builds have no
  Vulkan, so they aren't affected.
- **Desktop builds carry the same risk (#76).** The CPU fallback covers a missing GPU, a full GPU and a clean failure. An
  exception thrown inside ggml aborts the process instead, so a GPU driver that fails this way would crash the app.
- **Voice windows on the S23 cost about 0.15 times audio length**, including loading the models, on one 11 s clip, so
  `small` plus windows should stay under real time. This has not been run in a live recording.
- **`deb-gpu` VM (`vm-gpu`, outside this repo).** Vulkan through Venus reaches the host's GPU from Debian. Our engine there picks
  the NVIDIA card over the Intel one: `large-v3-turbo` 0.065 on the GPU against 1.22 on the VM's CPU. VFIO passthrough was not
  done: GNOME and open apps hold the card, it needs `sudo` and a reboot, and RAM snapshots wouldn't work with it.


## 2026-11-01 — Android check of #67 and the final pass in real recordings

- **Settings work on the S23.** Downloading `tiny-q5_1`, choosing it, recording with it, going back to `base` and removing it
  all worked. A fake key saved, was refused by OpenAI through Test key, and was removed. Not checked: a real key, a real
  OpenAI transcription, or the sealed file's contents.
- **The final pass runs in a real recording.** A temporary logging build reported `Meeting speakers finalized` with one
  speaker for one voice played from a speaker, and no failed chunks. One 12 s chunk took 2.6 s in all with `tiny-q5_1`, of
  which 0.74 s was transcription, so the live cost stays under real time. This was one voice on one device, so it says nothing
  about telling people apart on a phone. Which core speeds the app reads is still not observed.
- **Found: finished recordings show "Unattributed".** `ffi.rs` and `export_manager.rs` pick the label from the speaker identity
  alone, so a segment in an unknown speaker's group, which has no identity, gets "Unattributed" and not "Unknown speaker".
  The grouping itself is stored. This isn't Android-specific. Not fixed yet.
- **Found: Android apps write no logs.** No tracing subscriber is set up outside tests, so these messages are lost on phones.
- **Test leftovers.** The app has no way to delete a meeting or an unknown voice, so three `test*-delete-me` meetings and
  their unknown voices remain on the S23.

## 2026-11-01 — phone GPU retry with `base` (#75)

- **The Adreno 740 can't be used for now, and the error is known.** On the S23, `base` finished one pass of the 11 s clip at about
  0.6 times audio length, against 0.14 on the CPU, and its whole transcript was ".". The next pass aborted with
  `vk::Queue::submit: ErrorDeviceLost`. Two runs behaved the same way. Android builds stay on the CPU.
- **Not retried.** `small` and `large-v3-turbo`, which aborted earlier during model setup; that abort's text is still unknown, and no
  copy of those models was at hand. A driver update or a newer whisper.cpp could change this result.
- **This is the failure #76 describes.** A lost device during inference throws through Rust and ends the process, so the CPU fallback
  doesn't help when it happens on a desktop.

## 2026-11-01 — Why Adreno failed, and a working OpenCL path (#75)

This replaces the "can't be used for now" conclusion above, which only covered Vulkan through our build. Everything here ran in
a standalone test program on the S23 with a patched private copy of whisper.cpp 1.8.3. Nothing has shipped. All figures are from
one 11 s English clip, a few passes at a time. Battery use, sustained heat, Italian and `large-v3-turbo` weren't measured.

- **OpenCL works on the Adreno 740.** It gave correct text and no NaNs in any of 360 compared operations, against a median 0.14%
  difference from the CPU on each operation's largest value, and up to 4.5% among the non-trivial ones.
- **Flash Attention was broken by uninitialised memory.** The OpenCL kernel reads masked rows from the decoder's cache and
  computes `0 x NaN`. Guarding that fixes it. The `exp` guard was added first and didn't fix it alone; it's still in the patch.
- **Flash Attention isn't what makes it fast here.** The encoder took 15.3 s with it and 15.5 s without on OpenCL, and 1.30 s
  against 1.16 s on the CPU. It's still worth keeping on, because only that setting gave the exact transcript on OpenCL.
- **The real slowdown was the routing of matrix multiplies.** 95% of GPU time was in one generic kernel at about 2 GFLOP/s, while the
  Adreno-tuned path, which reads weights as textures, reached about 85. Sending large multiplies there took the `base` encoder
  from 15.5 s to 0.9 s. A faster tiled kernel also exists upstream but an earlier branch made it unreachable.
- **Vulkan remains broken.** It loses the device with every option tried. Its Flash Attention output is already wrong, and without
  Flash Attention some multiplies saturate at 65504. The cause isn't known.

| Model | Encoder CPU | Encoder GPU | Decode step CPU | Decode step GPU | Total CPU | Total GPU |
| --- | --- | --- | --- | --- | --- | --- |
| `base` | 1.32 s | 0.90 s | 7 ms | 27 ms | 0.14 | 0.21 |
| `small` | 4.47 s | 2.69 s | 19 ms | 48 ms | 0.46 | 0.46 |
| `medium` | 15.2 s | 8.16 s | 48 ms | 172 ms | 1.51 | 1.47 to 1.55 |

Totals are multiples of audio length, with 5 CPU threads and models with 16-bit weights.

- **The GPU encoder wins, the GPU decoder loses, so overall it's level.** An encoder on the GPU with a decoder on the CPU would be
  about 0.87 times audio length on `medium`, under real time. That figure is an estimate from the table and hasn't been run.
- **Shipping needs decisions.** It means carrying a patch to a third-party crate, adding the OpenCL build and Android's
  `uses-native-library` entry, and a fallback for other GPUs. The app hasn't been run with any of this.

## 2026-11-01 — Adreno decoder fix and CPU, CPU + GPU, GPU comparison (#75)

Same limits as the entry above: a standalone test on the S23, a patched private copy of whisper.cpp, and one 11 s English clip,
which is encoded as a full 30 s window. Not in the app. Battery use and Italian weren't measured. Each figure is the median of
4 passes, or 12 for sustained, with 5 CPU threads, Flash Attention on and cool-downs between setups. Transcripts were correct in all.

| Model | CPU only | CPU + GPU | GPU only |
| --- | --- | --- | --- |
| `base` | 0.143 | **0.108** | 0.141 |
| `small` | 0.496 | **0.313** | 0.343 |
| `medium` | 1.651 | 0.983 | **0.923** |
| `large-v3-turbo` | 2.789 | **1.393** | 1.463 |
| `medium`, 12 passes | 1.645 | 1.010 | **0.987** |

Figures are multiples of audio length.

| Model | Encoder CPU | Encoder GPU only | Decode step CPU | Decode step GPU only |
| --- | --- | --- | --- | --- |
| `base` | 1.32 s | 0.90 s | 7.6 ms | 17 ms |
| `small` | 4.82 s | 2.68 s | 21 ms | 31 ms |
| `medium` | 16.3 s | 8.07 s | 51 ms | 61 ms |
| `large-v3-turbo` | 29.8 s | 15.1 s | 14 ms | 19 ms |

- **Why the decoder was slow.** Decoder steps have one output row, but they were sent to tile kernels that process a block of 32 or
  more rows. Sending anything under 16 rows to the single-row kernel took `medium` from 172 ms to 61 ms a step.
- **The GPU decoder is still slower than the CPU on every model.** On `medium` the GPU is busy for about 36 ms of a step, with 636 kernel
  submissions. That leaves about 40 µs per submission, which is inferred from the timings and not measured directly. Qualcomm's
  performance hint didn't help. Recordable queues are untried.
- **CPU + GPU keeps only the encoder's weights on the GPU**, with no second copy of the model. It uses CPU cores and the GPU together,
  so it isn't clearly better than GPU only: the best setup depends on the model and the differences are small on `medium` and turbo.
- **What this means for live use.** `small` is comfortable. `medium` is borderline, at about 1.0 once warm and before voice windows,
  which cost about 0.15 more. Turbo is too slow to keep up, but about twice as fast as before for processing after a meeting.

## 2026-11-01 — Adreno decoder overhead is CPU placement, not submission cost (#75)
Same limits as the entries above: a standalone test on the S23, a patched private copy of whisper.cpp, and one 11 s English clip, encoded
as a full 30 s window. Not in the app. Battery use, Italian and running alongside recording weren't measured. Flash Attention on, 5 CPU
threads, cool-downs between runs, order rotated. Transcripts were correct in every run where they were checked.

| Model | Decode step: CPU | GPU, default | GPU, pinned | Encoder: CPU | Encoder: GPU |
| --- | --- | --- | --- | --- | --- |
| `base` | 9.3 ms | 17.8 ms | **8.3 ms** | 1.35 s | 0.92 s |
| `small` | 19.9 ms | 30.5 ms | **16.0 ms** | 4.41 s | 2.75 s |
| `medium` | 49.5 ms | 60.7 ms | **37.6 ms** | 15.1 s | 8.08 s |
| `large-v3-turbo` | 14.8 ms | 14.9 ms | **12.1 ms** | 28.3 s | 13.9 s |

Medians of 3 runs. Whole runs, as multiples of audio length, median of 3:

| Model | GPU, default | GPU, pinned |
| --- | --- | --- |
| `base` | 0.144 | **0.113** |
| `small` | 0.352 | **0.303** |
| `medium` | 0.913 | **0.855** |
| `large-v3-turbo` | 1.318 | **1.306** |

- **The cause.** Time inside `clEnqueueNDRangeKernel` fell from about 47 ms to about 18 ms per `medium` step when the threads were pinned, with
  the same 678 submissions. So the earlier "about 40 µs per submission" isn't a fixed driver cost. Merging kernels or recordable queues
  aren't needed to remove this overhead.
- **The fix.** During decoding, restrict the calling thread and the driver's `AdrenoOsLib` thread to one cluster, here the big cores 4–6. On
  `medium`, over 5 rounds: default 63.9 ms, driver thread only 42.0 ms, both 38.3 ms, all threads 38.3 ms. Pinning only the caller doesn't help,
  and the thread count has no effect. Pinning the whole process to a single little core still gave 42 ms.
- **Not proven.** Why placement matters. Handoffs between the two threads across clusters is a likely explanation, and the clusters and thread
  name are specific to this phone. Default runs occasionally reached 41 ms by chance, which fits placement being the cause.
- **What's left.** On `medium`, about 38 ms a step against about 36 ms of GPU busy time from the earlier profile, so little overhead remains.
  Further gains need faster kernels. The gain is clear on `small` and `medium`. On `base` and turbo the ranges overlap the CPU's, so call those level.
- **What this changes.** The GPU decoder no longer loses, so GPU only is now the simplest choice and needs no CPU + GPU split. Turbo barely
  moves because it has few decoder steps per run, and its encoder dominates. `medium` is still borderline for live use, once voice windows are added.
- **Before shipping.** The app needs patched whisper.cpp and OpenCL, which needs approval. Pinning must pick cores by capacity rather than
  a fixed mask, must tolerate a missing thread and offline cores, and should restore affinity afterwards. The prime core can be offline:
  setting affinity to it alone returned `EINVAL` here.

## 2026-11-01 — Efficient mode and a suggested model in Settings (#75, #68, #76)

The owner asked for a per-device model choice with a suggestion, offering both the standard models and an "efficient" version needing
the Adreno adjustments, and so approved the patched whisper.cpp and OpenCL on Android.

- **Efficient mode is a switch, not a second list of models.** It uses the same files, so nothing downloads twice and the list stays
  readable. Off is the default, and the standard models behave as before. The switch appears only on a supported GPU. Settings
  were already per device, as `<db>.transcription-settings.json` doesn't sync. No further models are bundled: `base` stays the only one in the app.
- **whisper-rs-sys is vendored** in `third_party/`, with an unmodified copy committed first. A build-time patch would break a fresh clone.
- **OpenCL is opened at run time**, because a normal link stops the app loading where the library is missing. The manifest declares it
  with `uses-native-library`, not required.
- **A standard phone doesn't initialize the GPU.** Upstream compiled every kernel when listing devices, about 9 s even for a CPU model; that
  now waits for a model to use the GPU. With a supported GPU, standard mode still creates one idle OpenCL context. Other phones see no OpenCL at all.
- **Supported: Adreno 730, 740, 750, 830**, the generations whisper.cpp has tuned kernels for. **Only the 740 is tested.**
- **Pinning covers the whole `full()` call**, from Rust, with no whisper.cpp change, as the encoder doesn't mind. Cores are the fastest cluster
  of at least two, here 3–6, because the S23's prime core can go offline.
- **The crash guard (#76) covers every GPU.** A driver can abort or `exit(1)`, as ggml does on a kernel compile error, which Rust can't catch.
- **Suggestion rule, advice only.** Phones with 6 GB or more: `small` in efficient mode on a supported GPU, `small` on the CPU from 2.8 GHz, else `base`.
  Computers: turbo with 4 GB or more on a graphics card, `small` with 7 GB of memory, else `base`. `medium` is never suggested on a phone,
  as about 1.0 with voice windows leaves no margin. Telemetry-based proposals from #68 aren't built, so it stays open.

Measured through the real engine with word timing, on an S23, `small`, 11 s English clip, median of two rounds with cool-downs, 3 passes each:

| Language | Standard | Efficient |
| --- | --- | --- |
| Fixed | 0.44 | **0.31** |
| Auto | 0.85 | **0.57** |

- **Auto language costs about 0.25–0.4 more**, on either setup, on a clip this short. Earlier figures used a fixed language, so Auto recordings are slower than those suggest.
  Choosing the language is the cheapest speed-up available.
- **Checked in the installed release app on the S23.** Android lets the app open the vendor's library: Settings suggests `small` in efficient mode and
  names the Adreno 740. The suggestion's Download and Use buttons worked. In efficient mode the app held 933 MB of GPU memory, and a recording of the
  public clip played from a laptop speaker gave a transcript (0:35), with no crash warning in Settings. With the language set to Italian, the English clip came out in Italian.
- **Not verified.** Thread pinning in the app: the driver thread was seen only on cores 3 and 5, inside the chosen group, but the app's thread mask can't be read from
  outside. Speed in the app, as Android builds write no logs. Accuracy of that transcript. A real driver crash; the guard is tested with a simulated abort. Battery use.

## 2026-11-01 — #63 a final voice replaces live members on paired devices

- **Found by naming a voice across two VMs.** A cluster share merged by union, so the whole-meeting
  voice joined the live chunks on the other device. There the live durations could pass the 8 s rule
  even when the final pass had marked the speaker as not clean (#74), so a device could save a voice
  the recording device rejected. Devices also showed different speech times, and because a digest held
  only counts, one live chunk becoming one final voice was never sent.
- **Decision.** A share holding the `meeting-voice-<cluster>` member replaces the receiver's members,
  and after that live shares add only clips. Digests also say whether the final voice is present. The
  digest isn't sent between devices, so older peers still interoperate, and a device updated later
  corrects itself at the next connection, for clusters still unresolved. Already-named ones keep what they have.
- **Naming says when no voice was saved.** The name still applies, but the person won't be recognised
  later, which the screen did not say. A final voice needs 20 s of speech (`MIN_CLEAN_VOICE_WINDOWS`), so
  a short speaker is named without a saved voice.

## 2026-11-01 — #63 closed: voice gallery checked on each platform

- **Play had never worked on Windows.** `fs::canonicalize` returns `\\?\C:\...` there, and Media Foundation
  reads that as a network path (`80070035`). Review clip paths now use `std::path::absolute`.
- **Checked:** Linux and Windows desktops and a daemon, paired over a real network, with public and
  synthetic audio: Play, naming in both directions, an existing person, a saved voice recognised
  later on another device, and deleting people and a meeting everywhere. The owner checked Android on
  an S23 with their own voice: a 28 s recording named, then a 14 s one matched automatically. That fits the
  20 s rule for saving a final voice. The S23 was not paired, so Android-to-device sync was not run.
- **espeak voices are not a test of telling people apart.** Two different espeak voices matched saved
  espeak voices, so these runs say nothing about false matches between real people.

## 2026-11-01 — Linux builds target a Debian 13 sysroot

- **Cause.** The build with diarization needed glibc 2.43 from this machine, for only seven float
  functions (`acosf`, `coshf` and similar), so it would not start on Debian 13 (glibc 2.41).
- **Decision: build against a hash-pinned Debian 13 sysroot, with clang and lld.** Building inside a
  VM would make releases depend on a VM. Symbol shims would hide the next such function. Zig cannot
  link the prebuilt ONNX Runtime, which needs libstdc++. The builds fail if a file needs more than
  Debian 13 offers, so the problem cannot come back unnoticed. Debian 13 and newer are covered; older
  systems are not.
- **Pinned packages can leave the mirror** at a Debian point release. `fetch-debian-sysroot.sh` then
  stops at the download, and its header says how to update the pins.
- **Checked on the VMs with public or synthetic audio only:** on `deb-shell`, a 25 s file holding two
  voices gave two unknown-voice clusters in 11 s, on 2 CPUs. The transcript's words were in the database
  file before deletion and gone after it. On `deb-gui`, the desktop bundle loaded both model sets and
  recorded from the VM's audio device, which is silent, so the desktop's transcription of speech is
  covered by the same core but was not exercised there.

## 2026-11-01 — Deleting meetings and voices

- **Deleting spreads to paired devices and always wins**, as voices already did (#56). Automerge and
  cluster shares merge by union, so a device left out would restore the item, and the owner expects
  deleted notes and biometric data to be gone. There is no "delete on this device only".
- **Records live in SQLite, not the Automerge document.** A document keeps its history forever, and
  the point is to erase it. `deleted_items` holds an ID and a time, kept forever; the IDs are never
  reused. `NTD1` messages carry them. They hold identifiers only, so the encrypted transport is enough
  and no pair key is needed. Because records only grow, their count tells whether a peer is up to date.
- **A new revision floor.** Revisions were one past the newest stored, so deleting the newest meeting
  would reuse its number, and a watcher that had seen it would miss the next write.
- **Only the private attachment folder is removed.** An attachment linked with
  `confirm_attachment_link` points at the owner's own file, which must stay.
- **Deleting a person now removes their external account links.** Foreign keys are enforced, so such a
  person could not be deleted before. No UI could reach this until now.
- **Deleting while recording is refused**, because the running pipeline still writes to that meeting.
- **The CLI needs `--yes`** rather than a prompt, so scripts stay non-interactive and a bare command
  never deletes. The web dashboard uses a dialog focused on Cancel.
- **Deleted bytes are erased, not just unlinked.** Every SQLite connection sets `secure_delete = ON`,
  because the setting belongs to a connection. Existing databases are rewritten once with `VACUUM`,
  recorded as `secure_delete_vacuum`, to clear what earlier deletions left in free pages. A failed
  rewrite (a busy file, a full disk) is logged and retried at the next start, so it never stops the
  owner reaching their notes. This does not reach SSD wear-levelled copies, backups or Automerge
  history in other meetings; a device's disk encryption covers the first.
- **Not built.** An old app version on a paired device drops `NTD1` as malformed and keeps its copy;
  this device still refuses to take the meeting back.
- **Which voice is playing:** the card turns the primary colour and shows `Playing…`, and Play becomes
  Stop. The controller holds one playing ID, so at most one card is marked.

## 2026-11-01: Proposing a Whisper model from this device's speed (#68)

- **What is measured.** Each transcription record gains the audio's length, a nullable column, so
  older rows are ignored. No audio or text is added. The speed is processing time divided by audio
  length, over a model's newest 30 chunks.
- **Enough data** is at least 5 chunks and 120 s of audio for the current model. With less, nothing
  is proposed from measurements.
- **The rule.** Above 0.9 the model is falling behind, so propose the next smaller one. At or below
  0.6 there is headroom, so propose the next larger one if it is predicted to stay at or below 0.6,
  using relative costs tiny 0.6, base 1, small 3.2, medium 10.7, turbo 10.7. A phone stops at `small`.
  Those costs are estimates, and a model not yet measured here says so.
- **Accuracy is not measured.** Telemetry knows speed and energy only, so "larger" follows a fixed
  order of the models, not a measurement of the owner's transcripts.
- **Advice only.** Nothing changes or downloads without a press. A dismissed model is kept in the
  settings file and not proposed again. With OpenAI chosen there is no proposal, because the device's
  speed says nothing about it.


## 2026-11-03 — #82 one person's unknown voices are named once

Found on the S23 after a long lunch recording ("Pranzo"): the Voices screen listed 21+ unknown voices for one meeting.

- **Two causes.** The final pass deleted only the live clusters a transcript segment pointed to. A diarized turn that got no words
  (a laugh, a short noise) still made a live cluster with a review clip, so those stayed as 0.7–5 s entries. The session now
  remembers every live cluster its turns touched and the final pass deletes each one it doesn't keep. Separately, every
  unresolved cluster was its own card, so a person split across final speakers, recordings or meetings was named once per card.
- **The library groups voices; storage doesn't change.** Unresolved clusters are grouped on each listing by average linkage over
  their representative voices. Nothing about groups is stored or synced, and naming a group still resolves each cluster on its
  own, so paired devices need no new message and older peers interoperate. The Linux, Windows and Android apps and the web show
  one card per group, with the parts listed, playable, and untickable when one is someone else.
- **Cutoff 0.5, measured on the 15 evaluation recordings** (`measure_meeting_pass`, ECAPA, 33 final speakers of 3 people). One
  person's whole-recording voices in different recordings: median 0.85, 5th percentile 0.58, lowest 0.50. Different people:
  99th percentile 0.37, highest 0.41. Pieces of 5 windows (about 5 s): different people up to 0.49; the same person joins above 0.5
  72% of the time, 91% at 10 windows and 99% at 20. Grouping all 33 voices plus a 3-window piece of each multi-speaker voice:

  | Cutoff | Groups (3 people) | Groups mixing people |
  | --- | --- | --- |
  | 0.45 | 4 | 0 |
  | 0.50 | 6 | 0 |
  | 0.55 | 8 | 0 |
  | 0.60 | 12 | 0 |

  0.45 groups best here, but these are clean recordings of three people on one microphone, and a wrong merge names someone
  silently while a split costs one more naming. 0.5 keeps a margin above every different-person pair measured.
- **Not covered.** Noisy rooms like the lunch recording, many speakers, and similar voices. Whether 0.5 still keeps people apart
  there needs real recordings (#72). The CLI still names one cluster at a time.

## 2026-11-03 — #83 leftover voices are deleted, and long recordings keep fewer tiny speakers

The S23's 14.5-minute "Pranzo" lunch with 5–6 people still listed 30 unknown voices after #82: 13 leftover pieces
(0.3–4.9 s) from before #82, 7 large voices (18–151 s) and about 10 small ones (3–12 s).

- **Leftovers are deleted, not hidden**, at the owner's request. At app and daemon start, before P2P, every unresolved
  voice of a finished recording that no transcript line points at, and that is not a final-pass voice, is deleted through
  `delete_unknown_voice`, so a deletion record stops paired devices sending it back. A recording counts as finished
  once a line points at a final-pass voice: the transcript is then the final one, so nothing will point at the others.
  Meetings without that (a crash, no diarization, a transcript not yet synced) and meetings being recorded are untouched,
  because a live voice there may still get its line.
- **A long recording needs more speech per speaker.** One person's voice drifts over a long meeting, and the final pass
  kept any group of 8 windows. Joining the 15 evaluation recordings into one 19-minute, 3-person meeting (clean audio)
  reproduces it:

  | Least group | Speakers found (3 people) | Windows misplaced (of 1145) |
  | --- | --- | --- |
  | 8 | 7 | 25 |
  | 15 | 5 | 26 |
  | 20 | 4 | 25 |
  | 30 | 3 | 22 |

  The least group is now 1/30 of the windows, at least 8 and at most 30. Recordings under 4 minutes, including all 15
  evaluation recordings, are unchanged. **The price:** in a long meeting someone who says less than that (20 s in a
  10-minute meeting, 30 s from 15 minutes up) is given to the nearest voice. Adding 10, 20, 30 or 45 windows of a third
  person to a 330-window two-person meeting kept them at or above the least group and folded them below it.
- **Not covered.** Recordings already finished keep their speakers; only new recordings are regrouped. A noisy room may
  still split one person into several voices of 30 s or more, which the voice groups of #82 then have to join.
- **Retuned on a second lunch (2026-11-03).** A 5-minute lunch of 3 people (231 windows, S23 microphone) still gave 6
  speakers. Two changes bring it to 3 (103, 83 and 45 windows), and leave all 15 evaluation recordings with the same
  speaker for every window:
  - The share is now **1/15** (at least 8, at most 30 windows), so a 5-minute meeting needs 15 windows: 4 speakers.
  - **A brief group heard only in isolated seconds joins its nearest group.** People talk in runs. Contiguity, the share
    of a group's windows with a window of the same group starting within a second, was at least 0.76 for every speaker
    of the evaluation recordings and 0.88–0.96 for the lunch's three people, but 0.53 for the fourth group, 15 single
    seconds spread over the whole meeting. A group under 30 windows with contiguity below 0.65 now folds. Its
    consistency (0.47–0.53 against 0.49) could not tell it apart. Long meetings that are sampled skip this rule, since
    sampled windows are not consecutive.
  - With either change alone the lunch kept 4 speakers.
  - **The price:** someone who says a few isolated words across a meeting, never a whole sentence, is given to the
    nearest voice.

## 2026-11-03 — #81 the tests that failed on a busy machine wait for progress or count CPU time

Under load (twice as many busy loops as cores, or a release compile alongside) the full `cargo test --lib` failed in 5 of
6 runs. Each failing test was checked for a real race before its check was changed.

- **The two speed checks** (1000 voice-sample seal/open round trips under 1 s; PDF font embedding under twice the
  built-in fonts) timed themselves by wall clock, which counts every moment another process holds the core. They now
  count the test thread's own CPU time (`cpu_stopwatch_for_tests.rs`, `CLOCK_THREAD_CPUTIME_ID` through the existing
  `libc` dependency; wall clock where a platform has no per-thread clock). Both pieces of work run on the test thread,
  so nothing is missed. The budgets are unchanged.
- **The two relay checks were snapshots, not races in the code.** "The destination refuses a device it is not paired
  with" asserted, 2 s after reservations, that the refused device was not connected. The gate refuses at the handshake,
  so the destination never admits it, but the dialing side sees each attempt open briefly, and it redials every 200 ms.
  The test now checks the destination's side on every step until the device has tried at least three circuits. "A
  stranger admitted for pairing cannot use the relay" asserted the stranger was gone 300 ms after the refusal, while it
  kept redialing, which the open pairing window rightly admits. It now stops redialing and waits for the relay to drop
  the refused connection. Both were checked against a broken gate: one that admits everyone, and one that never
  disconnects a refused client. Each still fails, with its own message.
- **The attachment sync test** didn't fail in 40 runs of its module under load. Its drives already wait for their
  condition, but gave up after 5 s, which a debug build moving a 200 KB attachment through SQLite can need on a disk busy
  with a compile. The limit is now 60 s, which only bounds a hang.
- **Result:** 50 of 50 full runs passed with a release compile of the core looping alongside (load average about 38 on
  22 cores), against 5 failed runs of 6 before.

## 2026-11-03 — #84 Devices moves into Settings; #85 approvals for Projects

- **Devices is a page of Settings** in the desktop and Android apps and on the web, first in the list. Pairing is a setup
  task, and Projects (#85) needs the tab, so the phone's bar stays at five: Record, Meetings, Projects, Voices,
  Settings. The web page keeps its `/devices` address.
- **Approved by the owner for #85:** `mail-parser` (Rust, pure, no network) to read `.eml` emails; `markdown` (pure
  Dart) and `pdfrx` (PDFium) to show Markdown and PDF inside the Flutter app. Emails come in as `.eml` files, and on
  Android through the share sheet; no mailbox connection.

## 2026-11-03 — #85 Project resources: sync, deletion, e-mails

- **One owner per attachment:** `SharedAttachment.owner` is a meeting or a project. Project resources reuse the `NTA1`
  manifests and the chunked transfer, with an empty `meeting_id` and a `project_id`, and travel in messages of their
  own: a device from before projects refuses an entry with no meeting ID, which drops the whole message, so keeping
  them apart lets it still receive every meeting attachment.
- **Deleting one resource** writes an `attachment` record in `deleted_items`, sent as `attachment_ids` in `NTD1`
  (missing in older messages, ignored by older devices). Like every deletion it always wins: a deleted resource
  counts as held, so it is never fetched again, and a row for it is refused. Meeting attachments are not deletable
  this way.
- **`mail-parser` is pinned to 0.11:** 1.0 needs Rust 1.98, above the project's `rust-version` (1.88).
- **E-mails stay whole:** the `.eml` file is the resource; its attachments of an accepted type are added to the same
  project as separate resources, and others stay inside the e-mail. HTML-only mail is shown as text, so no markup
  or remote content is ever rendered.

## 2026-11-03 — A lost pair key is asked for again

- **Found on the test VMs (#85).** The desktop app and the deb-shell daemon paired, but projects
  never reached the app. The daemon confirmed first and sent its agreement key before the app had
  admitted it, so the app dropped the key. Each side sent its key only once, at pairing or
  connection, so the app never got it. Every project list, voice library and attachment list
  sealed with that key was dropped too, and was not sent again because the sender had marked it
  as sent.
- **Decision: keys are answered, asked for, and shares are resent.** A device answers a key it
  hasn't answered on this connection with its own; a device that receives a sealed message
  without the sender's key sends its own key, which makes the sender answer; and whenever a key
  arrives, everything sealed or counted for that device is sent again on the next tick. Each
  device answers once per connection, so keys never bounce back and forth. The key message format
  is unchanged, so older devices still pair.

## 2026-11-04 — Meetings keep their transcription languages

- **Asked for:** a meeting's details name the language chosen for speech to text (#62), next to its
  date and time.
- **Stored in the meeting document**, as a map from code to the meeting offset where it was first
  used, so the language reaches paired devices and two devices recording one meeting merge cleanly;
  the earliest offset wins and orders the list. SQLite keeps a projection in
  `meetings.transcription_languages`.
- **Only stamped chunks count:** a chunk without a chosen language (the CLI and daemon using the
  environment's) records nothing, and meetings from before this change show no language. `auto` is
  recorded as `auto` (shown as "Automatic"), not as the language Whisper detected.
- **Shown** in the apps' meeting detail and on the web meeting page, which also gains the creation
  time in UTC, as the apps show it.

## 2026-11-04 — The CLI chooses a meeting's language; the configured one is recorded too

- **Asked for:** the CLI and daemon keep `NOTE_TAKER_WHISPER_LANGUAGE` as the general setting, a
  recording can choose its own language, and the language actually used is in the meeting's details.
- **`record start --language <code>`** overrides the variable for that recording.
- **The configured language is stamped on the audio:** the recording controller labels chunks
  without a chosen language with `NOTE_TAKER_WHISPER_LANGUAGE`, so the label travels with the audio
  (also to a paired device that transcribes it, which used to apply its own default) and is
  recorded like a chosen one. The pipeline records the same default for Teams-bridge chunks, which
  do not pass through the controller.
- **Stricter variable:** `NOTE_TAKER_WHISPER_LANGUAGE` must now be a code Whisper knows (it accepted
  any 2–3 letters), so a typo stops start-up instead of failing every chunk.
- **The daemon has no recording API**: it records only Teams meetings, which use the variable. A
  per-meeting language there would need the Teams bot to send one.

## 2026-11-06 — #87 After-meeting transcript pass, step 1 (desktop and CLI)

- **Why:** live chunks are about 11 s, so Whisper hears little context and words at the cuts
  suffer. On the evaluation sets, re-transcribing the speech in windows of up to 30 s with the same
  `small` model lowered word error from 41.8% to 40.4% (NOTSOFAR, 14 of 20 recordings better) and
  from 49.8% to 46.6% (AMI, 4 of 4), on par with or better than WhisperX `small`. Including the
  silent chunks changed nothing (40.3%, 46.5%), so only speech is retained.
- **Raw audio is kept, with the owner's approval** (an exception to CONTRIBUTING.md §7): encrypted with
  AES-256-GCM under the database key, bound to its meeting, only until the pass ends. It is
  deleted after the pass, when unreadable, when the meeting is deleted, or after three failed
  attempts, so it never lingers.
- **Speakers are not recomputed:** each new word takes the final speaker pass's speaker at its
  midpoint, so names and unknown voices stay as they were.
- **When it runs:** a dedicated thread, never while recording; on phones only while charging. The
  model is loaded per round and dropped. Android keeps it off until a charging job exists (step 2);
  a paired desktop doing it (step 3) and a Settings switch (step 4) follow.
- **Setting:** `NOTE_TAKER_TRANSCRIPT_PASS` and `NOTE_TAKER_TRANSCRIPT_PASS_MODEL`, also in the saved
  transcription settings, so a desktop can use turbo for the pass while live stays `small`.

## 2026-11-06 — #61 A voice needs up to a minute of speech in a long meeting

- **Why:** the final speaker pass still split one person into several voices in long meetings. A
  52-minute recording of 3 people gave 8 speakers.
- **Change:** the most windows `min_group_share` can ask a group for (`max_min_group_windows`) rises
  from 30 to 60, so from about 15 minutes of speech on a voice needs a minute to stand alone.
  Shorter meetings are unchanged.
- **Measured with the Rust pass** on 14 + 14 AMI and NOTSOFAR recordings, split into a tuning and a
  check set, and the 52-minute recording:

  | Cap | Confusion, tune | Confusion, check | Speakers found / real, check | 52-min recording |
  | --- | --- | --- | --- | --- |
  | 30 | 11.3% | 8.0% | 5.14 / 4.71 | 8 / 3 |
  | **60** | **9.8%** | **6.8%** | 4.43 / 4.71 | 4 / 3 |
  | 90 (Python copy) | 8.8% | 6.2% | 4.21 / 4.71 | 4 / 3 |

- **No real speaker was lost by the change.** The speakers missing at every cap (2 on the tune set
  and 9 on the check set, all with over 30 s of speech) are missing because their voice mixes with
  another, not because of the cap. 90 lowered confusion further but lost one more speaker, so 60
  was chosen.
- **The price:** in a meeting of 15 minutes or more of speech, someone who says less than a minute
  (in a long enough meeting; 1/15 of the speech before that) is given to the nearest voice.
- **Not changed:** the similarity threshold (0.15, 0.25 and 0.3 were all worse) and merging groups by
  their centres (worse on the check set).

## 2026-11-07 — #87 The pass caps each word at one second

- **Why:** Whisper's word times often run on to the next word, so a word before a silence seems to
  last through it, and in the pass's 30 s windows those silences are long. Such a word took the
  speaker of the next turn (its midpoint fell there) and stretched segments over silence. On the
  4 AMI test meetings the pass raised speaker error from 48.2% to 52.5% (false alarm 9.5% to
  21.0%, confusion 9.7% to 13.6%).
- **Change:** a re-transcribed word lasts at most 1 s from its start (`WORD_MAX_MS`). A replay over
  0.5–1.5 s caps, kept from the start or from the end, found every variant within 0.6 points on
  AMI and 0.2 on NOTSOFAR. 1 s keeps slow words whole.
- **Measured end to end** (CLI, live `small`, then the pass with `small`; with the speaker change
  of #61):

  | | AMI speaker error | AMI confusion | NOTSOFAR speaker error | Word error, AMI / NOTSOFAR |
  | --- | --- | --- | --- | --- |
  | Live | 46.9% | 8.4% | 42.6% | 49.8% / 41.8% |
  | Pass, before | 52.5% | 13.6% | 41.9% | 45.2% / 40.1% |
  | **Pass, now** | **44.0%** | 10.7% | **41.8%** | 45.2% / 40.1% |

- **Not tried yet:** whisper.cpp's DTW word alignment, which may place words better still.

## 2026-11-07 — #88 One meeting recorded on several devices

- **Why:** the owner wants to place the phone near the person they talk to and keep the laptop
  near themselves, and get one better transcript.
- **Recognised automatically, from the live transcripts:** shared three-word runs within 15 s of
  wall-clock time. On NOTSOFAR, the same meeting shared at least 60% of words, different meetings
  at most 3.6%; the threshold is 25% and 15 words. Shared words alone proved brittle (owner's
  review): devices at opposite ends of a table may understand mostly different words, and a phone
  left recording into the next conversation shares the end of the meeting. So the recordings'
  spans must also overlap for half of the shorter one, and when they cover nearly the same time
  (80%), 10% of shared words suffice, still at least 15. Speech-timing masks (Jaccard, correlation) did
  not separate meetings, and comparing audio would send audio before the pair is known.
- **Combined automatically, shown as one meeting** (owner's choice): the later recording is
  folded into the earlier; the list shows "recorded on Laptop + Phone"; Split undoes it for good.
  The folded recording's live transcript is kept, hidden, so Split restores it.
- **One device combines:** a desktop that finds the pair names itself; a phone names the other
  device. The other device sends its sealed speech over P2P (pair-key sealed parts with receipts)
  and deletes it after the last receipt. Speech not sent or combined within three weeks is deleted (24 h at first;
  raised on the owner's request so a phone away from the laptop for a while still delivers).
  Recordings are compared for folding over the same three weeks.
- **Late combining** (owner's request: "defer it when the data from the phone and laptop are both
  available, up to 3 weeks, encrypted meanwhile"): a recording no longer deletes its speech after
  its pass alone; with P2P it keeps it sealed, marked as passed, until combined or for three weeks.
  The cost is sealed raw audio on disk for up to three weeks per meeting (about 115 MB per hour of
  speech), on every device with P2P.
- **Any number of devices:** a recording joins a group through whichever recording it resembles,
  always into the group's first recording. The combining device is derived, not chosen: the
  first recording made off a phone, from a `recorded_on_phone` flag each device writes on its
  own meetings, so devices that fold at the same time cannot disagree.
- **Status in every app** (owner's request): a detail line under "Recorded on", derived on the
  showing device from the synced meetings, a `combined_with` list written after a combined pass,
  and the file times of the sealed speech (never the audio). Paired devices' storage IDs are kept
  with the pairing so a device that is away keeps its name.
- **How the transcripts are combined:** each speaker's words come from the device with more words
  for that speaker. On NOTSOFAR's far-field devices (pipeline-limits report, section 4.3), one
  device scores 36.9% word error, the best device per meeting 34.6%, the device that wrote the
  most words 36.2%. Merging every word added duplicates (about 3 points worse), and picking the
  louder device gained nothing; the per-speaker choice scored the same as the single device it
  was compared with. The union across devices (20.3%) bounds what a well-placed phone can add.
- **Device IDs:** meetings carry a storage ID (`device-<uuid>`), P2P a libp2p peer ID. Device
  manifests now carry the storage ID (optional, older devices still parse), and the P2P task
  translates.
- **Deleting a combined meeting deletes the recordings folded into it**, which would otherwise
  reappear alone.

## 2026-11-08 — #88 Kept speech is compressed with Opus

- **Owner's request:** compress the speech kept for the after-meeting pass, both on disk and
  before it is sent to the combining device. Raw speech took about 115 MB per hour, kept for up
  to three weeks per meeting on every device with P2P.
- **Codec and crate:** Opus, at 16 kHz mono in 20 ms frames, voice signal, complexity 10. The
  crate is `unsafe-libopus` 0.2.0 (BSD-3): libopus 1.3.1 translated to Rust, so Android and
  Windows builds need no C toolchain (there is no cmake here). It runs about 20% slower than C.
  The pure-Rust rewrites were too immature. The owner's request was taken as approval for the
  dependency. The unsafe calls are confined to two owned handles in `opus_speech.rs`.
- **Bitrate: 24 kbit/s**, about a tenth of the raw size (11 MB per hour). The measurement used
  the 20 NOTSOFAR recordings of the offline-small baseline (about 2 hours): each was transcoded
  in 30 s chunks, transcribed with Whisper small (greedy), and scored. Aggregate WER was 41.96%
  for raw audio, 46.06% at 12 kbit/s, 45.97% at 16, 42.62% at 24 and 43.56% at 32. DER was
  within 0.6 points of raw at every bitrate. Below 24 kbit/s Opus narrows the band and loses
  about 4 points; from 24 up, the differences are within the run-to-run noise of greedy decoding
  (32 scored worse than 24).
- **Format:** a record compressed this way uses a new sealing context, `retained-audio:v2:<id>`.
  `v1` raw records written by earlier builds are still read, so speech kept across an update is
  neither lost nor deleted as unreadable. The kept Opus is sent as it is (parts `NTR3`), so it is
  never compressed twice. The receiver checks that it decodes before it stores it. A raw
  `v1` record is compressed once, when it is sent. Parts from builds before Opus (`NTR1`) are not
  recognised, so a pair with one device not yet updated keeps its speech (up to three weeks)
  until both are.
- **Off the capture path:** compressing a 30 s chunk took 0.4 s on the laptop and would take
  longer on a phone, inside the call that delivers captured audio while it holds the engine. So a
  `retained-speech` thread compresses and appends the chunks in order. Stop flushes it before it
  releases the pass, so the pass never reads a meeting missing its last chunks.

## 2026-11-08 — Transcription glossary (names and project terms)

- **Owner's request:** let the owner give names and project terms that Whisper should spell
  right. One glossary per device, kept in `<db>.transcription-settings.json` next to the other
  transcription choices. The apps (FFI), the CLI (`glossary`) and the web
  (`/api/settings/glossary`) all edit it. It is read fresh when each recording and each
  after-meeting pass starts, so nothing is rebuilt.
- **Prompt:** Whisper's initial prompt is read as text spoken just before the audio. The glossary
  becomes a bare list, `Marco Rossi, Kubernetes, OKR.`, with no words of any language around it,
  so language detection is not pulled towards English. Whisper keeps the end of a prompt longer
  than about 220 tokens, so the previous chunk's text comes first and the glossary last. Limits:
  60 characters per term, 500 in all. Remote transcription gets the same prompt as the job's
  previous text.
- **Echo guard:** prompting can make Whisper write the prompt back over quiet audio. A transcript
  that opens with two or more glossary terms in a row loses them, together with their word
  times. One term alone is kept, as it can be a real answer ("Marco.").
- **Measurement** (NOTSOFAR 20, 30 s windows as in the after-meeting pass, Whisper small, greedy;
  aggregates only): mean WER 40.38 without a glossary, 39.04 with an unrelated 10-term list, and
  38.17 with each meeting's own names and terms (capitalised mid-sentence words said at least
  twice in the reference, about 6 per meeting). Term recall went from 51.3% to 54.1% and
  60.5%. DER is unchanged. So a glossary that does not fit the meeting does no harm. The terms
  are taken from the references, so the relevant row is an upper bound for a well-chosen list.

## 2026-11-08 — #90 #91 Overlapping speech and transcript confidence

- **Owner's request:** show where a transcript is weak instead of presenting every word as
  equally sure. A segment carries three marks (`SegmentConfidence`):
  - overlapping speech;
  - an uncertain speaker;
  - the positions of uncertain words.
  The marks are kept in the meeting's CRDT document and in a nullable SQLite `confidence` column.
  They reach the apps, the web and the exports (Markdown and PDF write `Name (?)`, `word (?)` and
  "overlapping speech"; JSON carries the fields).
- **Uncertain words:** a word takes the probability of its least sure Whisper token, and is
  marked under 0.4. On NOTSOFAR with Whisper small, 42% of such words were wrong against 3.1% of
  words at 0.95 or more (pipeline limits, C5). Remote and cloud chunks give no probability and
  are never marked.
- **Overlap:** the windowed diarizer already decodes, frame by frame, how many local speakers are
  active. A stretch is overlap where at least half of the windows covering it hear two or more
  voices. Spans closer than 250 ms are joined, and spans under 200 ms are dropped. They are
  stored per meeting in meeting time; the combined pass shifts each recording's spans by its
  offset. A segment is marked once 500 ms of it, or half of a shorter one, lies in overlap.
- **Uncertain speaker:** a live voice match left for review, or a final pass whose nearest voice
  turn is itself uncertain or more than 2 s away.
- **Measurement** (NOTSOFAR 20, 10 ms frames inside the app's speech chunks; aggregates only):
  - 17.4% of speech is flagged.
  - Against utterance-level reference overlap (28.6% of speech): precision 91.6%, recall 55.8%.
  - Against word-level overlap (24.7%): precision 86.4%, recall 60.9%.
  - The flagged spans hold 64.8% of the one-stream loss, that is speaker-time beyond the first
    voice.

  So the mark rarely fires on single-speaker speech, and it points at about two thirds of where
  words go missing. It is a reading aid; nothing is recovered from the second voice.

## 2026-11-08 — #93 The owner chooses how long speech waits to be combined

- **Owner's request:** the three weeks that a recording's sealed speech waits for another device's
  recording of the same meeting become a setting.
- **Range:** 1 to 60 days, default 21.
  - A day is enough for devices that meet daily.
  - Beyond two months, keeping speech (personal data) for a pass that improves an
    already-readable transcript is hard to justify.
- **Storage:** the setting is kept per device as `keep_speech_days` in
  `<db>.transcription-settings.json`, beside the glossary. It is edited from the apps (FFI), the
  CLI (`keep-speech`) and the web (`/api/settings/retention`). It is not synced: each device holds
  its own speech, so each device decides how long that speech lives there.
- **Effect:** the worker reads the window at each round, both for planning and for how far back
  it folds recordings. Shortening the window deletes speech that is already older at the next
  round, which is what the owner asked for. A value edited out of range by hand falls back to
  three weeks with a warning.

## 2026-11-08 — #92 Meeting text is sealed at rest

- **Problem:** voice embeddings, review clips and retained speech were sealed, but transcripts,
  transcript segments and the meeting CRDT documents (which hold notes and every transcript
  field) were stored in clear, against CONTRIBUTING.md §7.
- **Choice:** seal them with the existing AES-256-GCM `EmbeddingCipher` and the database key.
  Each value is bound to its field and meeting (`note-field:v1:<field>:<meeting id>`), so a
  value moved onto another meeting's row does not open. SQLCipher was set aside: it is a new C
  dependency, and this machine has no cmake to build it. No SQL query filters or searches on
  these columns, so nothing has to move from SQL to Rust.
- **Migration:** text columns tell clear values from sealed ones by SQLite type (TEXT vs BLOB);
  documents by the sealed header. Every opening seals what is left in clear in one transaction,
  so a database written by an older build on a shared device is sealed at its next opening.
  `secure_delete` overwrites the freed pages.
- **Stays in clear:**
  - Titles, project names and attachment names, which the meeting list shows. The plan the
    owner approved keeps them readable.
  - Attachment files, which stay in private 0700/0600 folders so the owner can open them with
    other apps and the planned notes folder (#98) can mirror them. Sealing them is tracked in #99.
- **Cost:** sealing a value adds 32 bytes (header, nonce, tag) and one AES-GCM pass per save, negligible next
  to the Automerge encoding.

## 2026-11-09 — #95 Meeting summaries with the owner's language model connections

- **Problem:** the owner wants a summary per meeting from a model they choose: OpenAI, Anthropic,
  Mistral, or a server on their own network. They also want a prompt template with placeholders,
  a prompt per meeting, and meeting photos sent along.
- **Protocols:** two adapters, both on blocking `reqwest`, called off the UI thread and outside
  the runtime lock.
  - OpenAI-compatible Chat Completions covers OpenAI, Mistral and local servers (Ollama,
    llama.cpp, LM Studio, vLLM).
  - Anthropic speaks its own Messages API, because the compatibility layer drops images and
    refusals.
  - No SDKs: the Rust core already has `reqwest`, and the stack rule forbids new frameworks.
- **Addresses:** HTTPS always. Plain HTTP is allowed only for loopback, RFC 1918 and ULA
  addresses, and the UI warns when it is used. Redirects are not followed, and an error body is
  never read or logged, so a key or meeting text can't leak through one. (Changed by #104 on
  2026-11-10: a rejection's short reason is now read and shown, never logged.)
- **Keys:** write-only, one keychain entry per connection (`note-taker-llm-<id>`). Android, which
  has no keychain, uses a file sealed under the host key. The CLI reads a key only from
  `NOTE_TAKER_LLM_API_KEY`, so it stays out of the shell history.
- **Settings:** connections, the default connection, the template and `auto_summarize` live in
  `<db>.llm-settings.json`, one file per device. They don't sync, since keys and LAN addresses
  are per device.
- **Where summaries live:** a summary is stored in the meeting's CRDT document (root key
  `summary`, with the connection, model, time and the prompt actually used), as is the meeting's
  own prompt (`summary_prompt`). So both sync to paired devices and are sealed at rest (#92).
- **Long meetings:** the budget is about 3 characters per token of the connection's context. A
  transcript that doesn't fit is condensed in parts, then the parts are combined, for up to
  three rounds; past that the meeting is refused with `too_long`.
- **Images:** photos and slides are sent only when the server accepts them. On a 400, 415 or 422
  the request is retried once without images. Downscaling pictures needs the `image` crate as a
  direct dependency, which still waits for the owner's approval; until then, pictures over 3.75 MB
  are skipped and counted in the result.
- **Tests:**
  - Every adapter test runs against a local axum mock.
  - End to end, the CLI transcribed the public-domain `jfk.wav` and summarized it through a
    local mock server. The request carried only the sample's transcript and no auth header.
  - No meeting content was sent to a cloud provider.

## 2026-11-09 — #101 Finished meetings are summarized automatically

- **Problem:** the owner wants every meeting summarized without asking, with an opt-out in
  Settings, while keeping the manual Summarize.
- **When:** a background round (at start, every five minutes, and when the app stops a recording)
  summarizes a meeting once its transcript is final. It must not be being recorded, and its
  speech must not be waiting for the after-meeting pass (#87, #88). A summary written before
  that pass would describe a transcript about to be replaced. A meeting folded into another is
  left to the meeting it joined.
- **Which device:** only the device that recorded the meeting. Paired devices would otherwise
  each send the meeting to a server and pay for it. The summary reaches them in the synced
  meeting document.
- **Never replaced:** a meeting with any summary is skipped, whether written by hand, by another
  device, or edited by the owner (#96). Writing a new one is a manual action.
- **Limits:**
  - Only meetings started in the last 7 days. Turning the setting on must not send the whole
    history to a server.
  - After 3 failures in a process (no transcript, server down, too long), the meeting waits for
    a manual summary. That way a broken connection doesn't retry forever.
  - Nothing is sent while the setting is off or there is no default connection.
- **On by default:** the summary of a recorded meeting is what the owner asked for. Nothing
  happens until they add a connection, which is itself the opt-in to sending meetings to that
  server.
- **Verified:** with a local mock server, the daemon summarized a newly recorded public-domain
  sample at start, with one request.

## 2026-11-09 — #96 The owner edits notes, summaries and transcripts

- **Where edits live:** in the meeting document, like everything else the owner writes. They
  sync to paired devices and are sealed at rest (#92). Notes are the document's `note_text`, in
  Markdown.
- **Summaries:** an edited summary keeps its connection and model and is marked `edited`. A
  summary written by hand has no connection or model and shows as "Written by you". Automatic
  summaries never replace either (#101).
- **Transcripts, one passage at a time:** a passage keeps its speaker, timing and overlap mark.
  Its unclear-word marks are dropped, since they described the old words. Empty text removes
  the passage. Free editing of the whole transcript was rejected: it would lose the speakers and
  times that the timeline, photos placement and exports rely on.
- **Edited transcripts are final:** the first correction sets the document's
  `transcript_edited` flag. The after-meeting pass and combining then leave the transcript alone
  and delete the retained speech, so a re-transcription can't undo the owner's corrections.
- **Surfaces:** core (`meeting_edits.rs`), REST (`/notes`, `/segments`, PUT `/summary`), FFI
  (`note_taker_meeting_edit`), web and app. The app edits a passage by long press, right-click
  or a screen-reader action, and Markdown in a full-page editor with a preview.
- **Limits:** 200,000 characters for notes and summaries, 20,000 for a passage.

## 2026-11-09 — #100 Search reads sealed text in memory, without an index

- **Problem:** the owner wants to find quickly where something was said or written: in every
  meeting, in every project, or in one project. Meeting text and documents are sealed at rest
  (#92), so SQL `LIKE` can't see them.
- **Decision: decrypt-then-filter, with no index.** Each search reads every meeting in scope
  (title, summary, notes, segments) and each project file's text through the repository, which
  unseals it, and matches in memory. Nothing it reads is kept.
  - The issue suggested an in-memory index rebuilt on change. It would need invalidation on
    every write and sync path, and it would keep the whole notebook unsealed in memory. A full
    scan of the scratch database answers instantly. An index can be added behind `search` if
    real libraries get slow, without changing any caller.
- **Matching:**
  - Case- and accent-insensitive (a small Latin fold, so no new dependency).
  - Every word of the query must be in the same passage: a transcript segment, or one line of a
    title, summary, notes or file. That is closer to "where was this said" than phrase matching,
    and more precise than matching words anywhere in a meeting.
  - At least 2 and at most 200 characters; at most 200 hits.
- **Where hits open:**
  - A transcript hit carries its time and speaker. The web links to `#t-<start ms>`; the app
    opens the meeting scrolled to the passage and marks it.
  - File hits open the file.
  - Snippets come as matched and unmatched parts, so no viewer has to count characters across
    UTF-8 and UTF-16.
- **File text:**
  - Text and Markdown files (their first 2 MiB) and e-mails (subject, senders, body).
  - PDFs and office documents are found by name only: extracting their text would need a new
    library.
- **Privacy:** the query and the text found are never logged. The web route and FFI read
  under the same repository lock as other reads.

## 2026-11-09 — #98 The notes folder: an opt-in plain-file mirror, last writer wins

- **Problem:** the owner wants projects and meetings as ordinary files, readable and editable
  with any editor, and new files dropped in to reach the app.
- **Decision: the store stays the source of truth; the folder is a mirror kept by rounds.**
  - Every 30 seconds, and on demand, a round reads edits back, imports new files, then writes the
    tree. Polling, with no file-watcher library. Edits made while the app was closed are seen at
    the next start, because what was written is remembered beside the database.
  - **Last writer wins.** A file edit becomes an ordinary edit in the store (the meeting document
    for notes and summaries, a replaced resource for project files), so it syncs like an edit in
    the app. An app edit made after it overwrites the file at the next round.
  - **Renames and deletions in the folder are not propagated.** The app writes the file back and
    never imports a renamed copy (recognised by size and time, or content up to 4 MiB). That
    rules out deleting notes by tidying a folder.
  - `transcript.md` is regenerated and read-only. Its speakers and times can't survive free
    editing; passages are corrected in the app (#96).
  - A file found where the app writes one, with other content, is kept as `<name> (on disk)`.
- **Off by default, per device, desktop only.** The folder is a decrypted copy of sealed data
  (#92), so turning it on shows that warning, suggests an encrypted disk, and refuses a
  filesystem root or the home folder itself. Folders are created owner-only (0700).
- **Attachment sealing (#99) is unaffected:** the folder copies attachments as stored. Sealing
  them later only changes how the copy is read.
- **Cost:** a round reads every meeting through the repository (like search, #100) and hashes the
  text files in the folder. That is fine for personal libraries. Rounds could later be driven by
  change notifications if it is not.

## 2026-11-09 — #97 Chat with notes: budgeted context, no index, history per device

- **Problem:** the owner wants to ask questions about one meeting, one project, or everything.
  With many meetings, everything does not fit any model's context.
- **Decision: gather, rank, fit, and say what was included.**
  - A meeting chat sends the whole meeting (summary, notes, transcript) and its photos.
  - A project chat sends the files' text first (usually short and to the point), then the
    meetings newest first, plus the newest 4 images.
  - The "everything" chat starts with a list of every meeting and project (titles, dates,
    projects), so questions like "when did we meet about X" can be answered without the
    transcripts. Then come the meetings and files ranked by how many of the question's words
    (4+ letters, case and accents ignored) they contain, newest first among equals.
  - `fit_context` gives everything when it fits. Otherwise it gives summaries and notes in
    place of transcripts in rank order, dropping the last items, then restores whole meetings
    in rank order while they fit. Each answer lists what was given and what was left out, and
    the model is told what was left out.
  - No embedding index or vector search: it would need a new model or library, and the
    word-overlap ranking with summaries fits personal libraries. It can be swapped in behind
    `gather_material` later.
- **History:** kept per device in SQLite, sealed per message (`chat-message:v1:<chat>:<pos>`),
  at most 200 messages per chat, not synced. Each question carries the most recent turns that
  fit a quarter of the room. Deleting a meeting or project deletes its chat.
- **Connections:** the same ones as summaries (#95); the apps use the default one, while the CLI
  and the API can name another. Material goes only to the connection the owner configured. Tests
  use fake engines; no meeting content reached a cloud provider.
- **Blocking:** hosts hold their store only while gathering and storing, never while the model
  answers, as summaries do.

## 2026-11-09 — #99 Attachment files are sealed at rest; viewers get short-lived clear copies

- **Problem:** since #92 notes, transcripts and documents are sealed in SQLite, but attachment
  files (meeting photos and files, slides, project resources, files from paired devices) were
  kept in clear in private 0700/0600 folders. CONTRIBUTING.md §7 asks for sensitive data to be
  encrypted at rest.
- **Decision (the owner chose it): seal every attachment file with the database key; readers
  decrypt.**
  - Format: `NTSEALF1`, a random file ID, then records of up to 256 KiB sealed with AES-256-GCM
    through the existing cipher. Each record's context names the file ID, its kind and position;
    a final record holds the clear length. Reordered, dropped, moved, cut or altered records are
    refused. Records keep memory bounded (100 MiB files) and let P2P serve a chunk at any offset
    without decrypting the whole file.
  - The file ID is in the file, not the attachment ID, so readers need only the path. Swapping
    whole files between attachments needs write access to the folder, which could delete them
    anyway.
  - Every core reader decrypts in memory: exports (staged clear copies, an explicit export),
    summaries and chat (images), project commands and web downloads, search, P2P sync and the
    notes folder (a decrypted copy by the owner's choice, #98).
  - P2P hashes, sizes and chunks are of the clear contents, sent over the encrypted transport;
    each device seals what it receives with its own key, part by part, so clear bytes never touch
    its disk.
  - The app's image, text and PDF views read files, so `note_taker_clear_copy` writes a clear
    copy into a private `clear-copies` folder. Copies are removed 30 minutes after they were made
    (when another copy is made) and the folder is emptied at every start. This is the smallest
    clear footprint that keeps the viewers; reading bytes over FFI into memory would need a
    second PDF path and copies of large files across the FFI boundary.
- **Migration:** files from before #99 are still read as they are. The app and the daemon seal
  them on a background thread at start (the CLI in line), each written beside the original and
  renamed over it, so a crash leaves either the clear or the sealed file. A file deleted while it
  is sealed is not brought back.
- **Not covered:** clear copies exist while a file is open in the app; the notes folder is clear by
  design; exports are clear by design. Verify on the owner's devices before closing the issue in
  their eyes: the first start seals the existing library.

## 2026-11-09 — #87 The phone reports charging to the after-meeting pass

- **Problem found on the S23.** A phone's pass control (`RefinementControl::new(true)`) waits for
  charging, but nothing ever reported it, so a phone never ran the pass alone or combined
  recordings. Folding, sending to a paired device and deleting still ran. Separately, the app
  starts the pass worker only with the meeting pipeline, on the first recording of a session, so
  speech left from an earlier session waited for the next recording.
- **Decision:** the platform reports charging through `note_taker_set_charging` (JNI
  `NativeCore.setCharging`); `ChargingReporter.kt` follows the sticky `ACTION_BATTERY_CHANGED`
  broadcast while `MainActivity` exists. Android SDK APIs only, no new library.
  - The state is kept in a static and copied into the runtime when it opens, because Kotlin
    reports before Dart has handed over the database key; the call never creates the runtime.
  - A pipeline built later starts with the last report.
  - Charging while the retained-speech folder holds speech builds the pipeline, which starts the
    worker without a new recording. Kotlin repeats the report on every battery change while
    charging, so this happens soon after the database opens. The build loads the model, so the
    call runs on a background thread.
- **Limit:** the pass runs while the app's process runs. Android may freeze a cached app soon after
  it leaves the screen, so a long pass can pause until the app is opened again. Running it fully
  in the background would need a foreground service or WorkManager (a new library), which is the
  owner's call.

## 2026-11-09 — #103 Online-meeting tracks may carry several voices; keeping voices is opt-in

- **Problem.** The bridge credited each participant's whole stream to that participant, on the
  assumption that one track is one person. In a hybrid meeting, a meeting room (or a shared
  laptop) joins as one participant with several people behind its microphone, so everything said
  in the room went to the room's name. `TeamsProcessor` also stored an embedding per chunk without
  any check. It is not wired into the bridge, but a room's blended voice in the gallery would spoil
  later matching. Meanwhile the live path kept no voice at all, so a remote participant was not
  recognised when the same person later sat in the room.
- **Decision:** each chunk of a participant's track is diarized. The policy lives in
  `domain/participant_track_voices.rs` and the session part in
  `pipeline/meeting_session/participant_tracks.rs`.
  - *One voice*: one voice holds at least 90% of the chunk's speech, or the others stay under
    2 s (noise, cross-talk). Turns are the same voice above cosine 0.40, the unknown-speaker
    clustering cut-off, so a track splits exactly where the app would see two people.
  - While a track carries one voice, the chunk goes to the participant, as before. The same
    happens without a diarization engine or when diarization fails, so the common case never
    gets worse.
  - A chunk with several voices, or a single voice clearly unlike the track's voice once that is
    known (5 s; people in a room taking turns), switches the track for good. From then on every
    voice is matched against the gallery like local audio: known people get their names, others
    join unknown-speaker clusters with review clips, which the owner names in the usual review.
    The switch is permanent, because a room that is quiet for a while is still a room.
  - **Voice gallery.** After 30 s of clean speech of the one voice (overlap excluded), the
    voice's mean embedding is offered once per meeting to `IdentityMapper::keep_participant_voice`,
    unless the gallery matches it (strongly or weakly) to someone else. A track with several
    voices is never offered. The bridge's `TeamsIdentityMapper` keeps it only when the owner turned
    on *Keep meeting participants' voices*.
  - **The setting** is off by default: voice prints are biometric, and the other participants did
    not install the app. It lives in the transcription settings file as `keep_meeting_voices` and
    is read at every offer, so switching it needs no restart. It is exposed as
    `GET`/`PUT /api/settings/meeting-voices`, the web Settings switch and CLI
    `meeting-voices [on|off]`. No Flutter switch yet: the bridge runs only in the daemon. The
    switch comes with the browser extension (#102), which may feed the apps. The consent notice
    (`docs/meeting-consent-notice.md`) already mentions voice prints.
- **Limits.**
  - Chunks credited before the second voice appeared stay credited to the participant. They are
    at most a few chunks, since a room is usually busy from the start.
  - A room's voices are labelled by the gallery or as unknown speakers, not as
    "<room> · Speaker A". Clusters already group them per meeting, and the review names them.
  - The thresholds come from the existing clustering cut-off, not from a measurement on
    room recordings. There is no multi-person track in the local datasets; AMI's mixed headset
    channel could serve as one if needed.
  - `TeamsProcessor` still stores an embedding per chunk. It is used only by its tests, and its
    doc says it must not be wired in.

## 2026-11-09 — #102 A browser extension records Teams meetings joined in the browser

- **Context:** the owner mostly joins Teams from the browser, on a work account
  (`teams.cloud.microsoft`) and a personal one (`teams.live.com`), and hosting the media bot
  (#34) on a Windows server is heavy. The #102 probes showed what a browser page gives:
  - one `<audio>` element plays every remote participant mixed into one track, and the microphone
    is the track Teams opens with `getUserMedia`;
  - live captions carry the speaker's name (and, on work accounts, Teams ID) and grow word by word;
  - while Teams transcription runs, its panel shows the transcript;
  - a screen share is a `<video>` the page can draw;
  - after the meeting Teams posts a recap card with the transcript (`.vtt`), but only to the
    organizer's organisation;
  - Debian's Firefox ESR turns off OpenH264, so it shows no screen share.
- **Decision:** an MV3 extension (`browser-extension/`, TypeScript, esbuild, Vitest) for Firefox
  128+ and Chrome/Edge 111+ talks to the daemon's existing Teams bridge through a native messaging
  host. The bridge refuses WebSocket handshakes with an `Origin` header, so pages cannot reach it.
  - **Protocol** (additive, the bot is unaffected):
    - `AudioChunk.track` is `PARTICIPANT` (the owner's microphone) or `MIXED_REMOTE` (everyone
      else).
    - `MeetingDocument` carries the captions, live transcript or Teams transcript (UTF-8, at most
      4 MiB, `text/vtt` or `text/plain`). It is stored sealed as a meeting attachment. It may name
      its meeting, so a transcript published after the meeting does not reopen it.
    - `MeetingEnded` runs the after-meeting pass.
  - **Mixed track:** it maps no identity, and its voices are matched against the gallery from the
    first chunk (#103's several-voices path). It is never credited to one person and never kept
    for the gallery.
  - **Host:** `browser-host` (Rust), registered by `browser-host install` for Firefox, Chrome,
    Chromium and Edge. It allows only the extension's IDs. It queues up to 4 min of audio while
    the daemon is down. The bridge secret lives in the keychain (`teams-bridge-secret`), because a
    browser-started host does not get the daemon's environment. The daemon reads it when its
    environment sets none.
  - **Consent:** the owner is asked when a call starts ("Always for Teams" stops asking), and an
    indicator with Stop stays visible. Starting Teams transcription is a separate opt-in, off by
    default: everyone sees it, and Microsoft stores the transcript.
  - **Teams transcript:** the extension never calls Microsoft's APIs and never uses the page's
    tokens. It opens the recap and copies the transcript the page itself loads, using Download →
    `.vtt` only if needed (which also leaves a copy in Downloads). The recap is matched to the
    recorded meeting by title and time. The extension keeps those for two weeks, nothing else.
  - **Slides:** a share frame is kept once it changed and held still, and is cropped to the slide:
    the largest rectangle with per-channel colour edges on four sides, a 1.2–2.0 aspect ratio and
    an even surrounding band. On the real test frame this gives exactly the slide.
- **Limits:**
  - The Teams controls are found by `data-tid`, ARIA label or English text. Another UI language,
    or a Teams update, can stop captions, transcription start or recap fetching (each gives up
    quietly) until the selectors are updated.
  - Caption times are when the caption appeared, about a second after the words.
  - The microphone maps to a `browser-owner` participant named after the Teams profile, not yet
    to the owner's own gallery identity.
  - Not yet verified live in a browser: the host disk filled up during this work and stopped the
    test VMs. (Verified on 2026-11-10 in Firefox ESR; see the next #102 entry.)

## 2026-11-10 — #102 What the first live test in Firefox changed

- **Context:** the extension was tested live in Firefox ESR 153 on Debian 13, on a work account,
  in a call with a second account sharing a slide deck. It recorded, but several parts failed:
  - nothing ran until site access was granted by hand;
  - the diarizer refused the bridge's PCM;
  - the Teams menus were not found;
  - the owner was named "Your profile";
  - reading the share froze the Teams tab (3 GB, killed);
  - slides with the same layout were not told apart.
- **Decisions:**
  - **Site access:** Firefox does not grant a hand-loaded MV3 extension its `host_permissions`.
    On install, the background opens the popup in a tab when access is missing. The popup asks for
    it with **Allow on Teams** (`permissions.request` needs a click).
  - **WAV for the diarizer:** external chunks are wrapped in a RIFF/WAVE header
    (`domain::pcm_wav`, `ExternalMediaChunk::wav_bytes`) before diarization and VAD. The port
    reads WAV like the app's own recordings, and adapting it was a smaller change.
  - **Automation:**
    - Controls are found by their stable IDs first (`LanguageSpeechMenuControl-id`,
      `closed-captions-button`, `RecordingMenuControl-id`, `call-transcript-button`).
    - A shortcut after a label ("Alt+Shift+C") is ignored.
    - An open More menu is reused, not toggled shut.
    - Steps run one at a time and close the menu after themselves.
  - **Owner:** the name comes from the avatar's "Profile picture of …" label. The participant ID
    is `browser-owner:<name>`. The app keeps the first name it saw for an ID, so a fixed ID kept
    "Your profile" for good and would have merged two owners of one browser.
  - **Slides in the page world:** the page hook draws the share and posts only a 64×36
    signature; on request it posts the cropped slide as base64 JPEG. Doing this in the content
    script was impossible in Firefox:
    - reading a page video's pixels there goes through per-byte wrappers, which froze the tab;
    - awaiting the page's promises from an `OffscreenCanvas` is denied ("Permission denied to
      access property constructor").
  - **Slide changes:** a slide counts as new when at least 8 of 64×36 cells change brightness by
    more than 12. Measured on the live frames: slides 1 and 3 of the deck differ in 47 such cells,
    the same slide in 0, and the pointer covers at most 4. The earlier mean over a 32×18 grid
    (0.82 against a threshold of 2.5) missed text-only changes.
  - **Progress in the popup:** audio chunks, slides and captions sent, and the last problem. A
    failure was otherwise invisible.
- **Verified live (deb-gui, Firefox ESR, work account):**
  - consent and the recording indicator;
  - microphone speech transcribed and credited to the owner by name, with no diarizer errors;
  - captions turned on and Teams transcription started automatically;
  - three slides kept for three slides, each cropped exactly to the slide;
  - the meeting ends when the page leaves the call.
- **Not yet verified:**
  - the Teams transcript fetched from the recap after the meeting (done the same day; see the
    next entry);
  - Chrome and Edge (Windows);
  - remote participants' audio (the second account was not speaking).

## 2026-11-10 — #102 The Teams transcript comes from the recap's SharePoint frame

- **Context:** after the live-test meeting ended, no transcript was fetched. Teams (work account,
  November 2026) no longer posts a recap card with "View recap" in the meeting chat. The chat now
  opens on a recap view:
  - a session picker ("Select the meeting by time");
  - **AI summary / Transcript** tabs;
  - the transcript in a frame served from the organisation's SharePoint (Stream's player).

  The extension ran only on Teams sites, so it never saw that frame. In that frame:
  - the player loads the transcript from `…/cdnmedia/transcripts?…&kid=…`, which is binary
    (encrypted for the player), so copying that response cannot work;
  - only **Download → Download as .vtt** fetches plain WebVTT
    (`…/media/transcripts/<id>/streamContent`).

  Also, an unselected Fluent tab holds its label twice ("TranscriptTranscript"), so the Transcript
  tab was never found.
- **Decisions:**
  - **SharePoint access:** the manifest adds `https://*.sharepoint.com/*`. A tenant's SharePoint
    host is not known in advance. The hook and a new recap frame script run there in all frames,
    but act only in a sub-frame whose referrer is a Teams site (`embeddedInTeams`). On other
    SharePoint pages they do nothing. The owner grants it with the existing **Allow on Teams**
    button.
  - **Download through the player:** the Teams tab detects the recap view and matches it to a
    recording by title and session time, as for cards. It then opens the Transcript tab and asks
    the background to have the tab's recap frames press Download → .vtt. The frame's hook copies
    the WebVTT, and the background relays it back to the tab, which sends it to Note Taker. This
    leaves a copy in the browser's Downloads (already accepted for cards). The extension still
    makes no request of its own and uses no page token.
  - **Once per meeting:** the player can deliver the file twice for one download. The tab marks a
    meeting as sent before any wait, so the second copy is dropped. The live test had stored two
    attachments before this.
  - **Retries:** a failed fetch is tried again a minute later, three times per page load. Before,
    one early failure (the recap still loading) stopped it for good.
  - **Status in the popup:** sent, already sent, no matching recording (with the session time),
    or the step that failed. The tab resends it, and its recording state, when Firefox restarts
    an idle background. The recap frame reopens its port then too, or download requests stop
    reaching it.
  - **Doubled labels:** controls are matched on their label read once (`withoutDoubledLabel`).
- **Verified live (deb-gui, Firefox ESR, work account):**
  - SharePoint access granted through the popup;
  - the recap view detected with its 16:10–16:21 session;
  - Download pressed in the frame, one `.vtt` downloaded, and one `teams-transcript` attachment
    stored in Note Taker;
  - the popup then said "already sent".

  A test record covering 16:10–16:21 was used. The recap of the later test call (17:33–18:18)
  had not appeared in the picker an hour later. The test record was removed afterwards.
- **Limits:**
  - Only the session selected in the picker is considered. Teams opens on one; older sessions of
    a recurring meeting are not browsed.
  - A recording's end time is lost when the Teams page is reloaded during the call. Matching then
    uses its start, with the 30-minute slack.

## 2026-11-10 — #104 Show the AI server's reason when it rejects a request

- **Context:** on the S23, **Summarize** with an Anthropic connection (`claude-opus-5-5`,
  `https://api.anthropic.com`, key saved) answered only "The AI server rejected the request.
  Check the model name." The model name was right, and the request matched Anthropic's current
  API. The cause was in the error reply's body, which the transport threw away so that no
  meeting text could leak through it. The owner could not tell what to fix: credit, a key
  without access to the model, a parameter.
- **Decision:** for a 4xx rejection (other than 401, 403 and 429), keep the server's own reason:
  - `error.type: error.message` (Anthropic and OpenAI), or `error` when it is text (Ollama);
  - collapsed to one line and cut to 300 characters; the rest of the body is dropped;
  - carried in `LlmError::Rejected.provider_reason`, outside the error's text, so logs (which
    use the text) never hold it;
  - returned as `error_detail` by the app's FFI and the web API, and shown under the message as
    "The server said: …".

  A local server could echo some meeting text in its reason. That text goes only to the owner's
  own screen, which already shows the meeting.
- **Also fixed:** Android, Windows and macOS builds failed since the Teams bridge secret moved to
  the keychain (`56b6972`): `SECRET_SERVICE_LABEL` existed only on Linux.

## 2026-11-10 — #105 Summaries through Claude Code and Codex on the desktop

- **Context:** the owner pays for Claude Pro and has Codex signed in with ChatGPT, but has no
  Anthropic API credit (#104). A subscription can't be used through an API key, and borrowing a
  claude.ai session would break Anthropic's terms. Claude Code and Codex, though, are the
  vendors' own tools, covered by those plans, and both run headlessly.
- **Decision:** two connection kinds, `claude_code` and `codex`, that run the installed tool
  once per request. Everything that uses connections works unchanged: manual and automatic
  summaries (#101), chat (#97) and the connection test.
  - **No address, no key.** Any key sent is ignored. `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`,
    `OPENAI_API_KEY` and `CODEX_API_KEY` are removed from the tool's environment, so the
    subscription login answers and API credit is never billed by surprise.
  - **The model is optional;** empty uses the tool's default. A name must be a plain model name
    (letters, digits, `-._:/[]`, not starting with `-`), since it goes on the command line.
  - **The tool can only write an answer.** Claude Code runs with `--tools ""`, so a transcript
    that reads like instructions can't make it run commands or read files. Codex runs in its
    `read-only` sandbox; it can still read files, which the input could ask for, but it can't
    change anything. The answer only reaches the owner's own app.
  - **Nothing left behind.** No session is saved (`--no-session-persistence`, `--ephemeral`).
    The run happens in a new 0700 temporary folder that is removed afterwards. The prompt goes
    on standard input, not the command line, where other processes could read it.
  - **Images:** Codex takes them (`--image=`). Claude Code without tools can't, so they are left
    out and counted as such (`LlmEngine::reads_images`).
  - **Finding the tool:** `PATH`, then the usual install folders (`~/.local/bin`,
    `~/.claude/local`, npm's folders, `/usr/local/bin`, `/opt/homebrew/bin`), since an app started
    from the desktop has a shorter `PATH` than a terminal. Settings shows "Installed" or "Not
    found on this computer".
  - **Desktop only:** the app offers these presets on Linux, macOS and Windows, not on Android.
    The web UI offers them too, since the daemon runs on a computer that may have the tools.
  - **Failures** (signed out, plan limit) come back as `llm_tool_failed`, with one short line of
    what the tool said; a missing tool is `llm_tool_missing`.
- **Verified:** on the owner's host, an ignored test ran both real tools through the engine on
  a made-up meeting: Claude Code answered in 2 s on the Pro login, Codex in 6 s on the ChatGPT
  login. No folder was left in the temp directory. Unit tests cover the command lines,
  environment, answers, failures, missing tools and the image rule.
- **Costs and limits:** these runs count against the plans' usage limits. Codex adds its own
  agent prompt (about 14k tokens even for a one-line request).


## 2026-11-10 — #87 The after-meeting pass is on for phones, with a Settings switch

- **Why:** the two meetings recorded on the S23 today kept no speech and were never transcribed
  again. The pass was still off by default on Android ("until a charging job exists"), and the
  charging job (a848227) had landed without turning it on. Nothing in the app could turn it on.
- **Decision:** on by default on every platform. A phone keeps the meeting's speech, encrypted,
  and runs the pass while charging, then deletes it, as the owner decided on 2026-11-06.
- **Switch (plan step 4, on/off part):** Settings → Transcription → "Improve the transcript after
  the meeting", saved as `transcript_pass` in the transcription settings file through
  `note_taker_set_after_meeting_pass`. It applies from the next recording; speech already kept is
  still processed and deleted. The pass model is not in Settings yet (`transcript_pass_model` or
  `NOTE_TAKER_TRANSCRIPT_PASS_MODEL` set it), and the web UI doesn't show the switch, since the
  daemon already had the pass on.
- **Cost on a phone:** speech is kept as Opus until the pass, and longer when it waits to be
  combined with another device's recording ("Keep speech for combining"). The pass itself is a
  second transcription of the meeting, so it runs only on the charger.

## 2026-11-10 — #106 "Automatic" is the default Whisper model choice

- **Why:** the owner wants the app to pick the right model by itself on any phone, while still
  allowing a model chosen by hand.
- **Rule:** Automatic uses the device suggestion (`suggest_whisper_model`: memory, GPU, top core
  speed), including efficient mode when it says so. It is worked out again each time a pipeline
  is built, so it follows the device rather than a saved answer.
- **No surprise downloads:** if the suggested model isn't on the device, Automatic runs the
  shipped model and Settings offers the download. Nothing downloads without the owner's say
  (#68); a phone may be on mobile data, and `small` is about 490 MB.
- **Stored as no choice:** `whisper_model` unset means Automatic. Existing installs that never
  picked a model become Automatic. A hand-picked model that was removed counts as Automatic.
  `selected_model` in the settings JSON no longer falls back to the shipped model.
- **Efficient mode:** Automatic decides it, so its switch shows only for a hand-picked model.
  The GPU crash guard still applies.
- **Telemetry proposals (#68)** are unchanged. They are based on the current model, Automatic's
  included, and accepting one picks that model by hand.

## 2026-11-12 — #110 Chat answers cite their sources as links

- **Why:** an answer couldn't be checked against the meeting it came from. Yogurt links each
  answer to its source; the owner asked for the same.
- **How:** the material sent to the model is numbered (`## [1] Meeting "…"`), and the model is
  asked to cite right after each statement, as `[n]` or `[n, hh:mm:ss]` for a moment in a
  transcript. The answer's text is kept as written; `parse_citations` reads the citations into
  sources (meeting or file, and the moment), each place once, in order of first mention.
- **Robust to the model:** numbers outside the material, Markdown links (`[text](url)`),
  checkboxes and other bracketed text are ignored, so a model that cites badly loses links, not
  the answer. A list (`[1, 2]`) cites both.
- **Stored** with the answer, sealed like the rest of the message; answers stored before have none.

## 2026-11-12 — #116 A voice's review clip holds only that voice

- **Why:** the owner couldn't name new voices: their clips held two people, so all of them had to
  be deleted. A clip was the whole 3–15 s chunk the voice was first heard in, and a chunk often
  holds several people. Also, the live pass can merge two people into one voice. The final pass
  then splits them, but the clip stayed with whichever speaker kept the cluster.
- **When a voice is first heard:** its clip keeps only its diarized span of the chunk
  (`slice_pcm16_wav`), timed from 0.
- **After the meeting:** the final pass cuts the clip again, to the longest unbroken stretch of the
  speaker it gave that cluster. Each voice window counts for its middle half, where its speaker is
  surest. Another speaker's window, or more than 0.5 s of silence, breaks a stretch. Under 1 s of
  that speaker in the clip deletes it. No clip is better than one of somebody else, and the voice
  can still be named from its transcript lines.
- **Older clips:** clips saved before this change still hold a whole chunk but record the voice's
  span. Playback (FFI and `GET /api/clusters/{id}/review-clip`) plays only that span.
- **Not changed:** speakers the final pass creates without a live cluster still have no clip. The
  final pass keeps no audio in memory. A clip for them could come from the after-meeting pass's
  retained audio; that is left for later.

## 2026-11-12 — #115 The app is called Koalino and licensed GPL-3.0-or-later

- **Name:** user-visible text now says Koalino. That covers the Android label, the Linux and
  Windows window titles and the Windows file description, the Flutter and web text, the browser
  extension and the Teams app manifest, and the READMEs.
- **Kept until the release** (owner: "keep the internal id for now"):
  - the applicationId `com.example.note_taker_ui`, binary and crate names, `NOTE_TAKER_*`
    variables and data folders;
  - the keychain and Secret Service labels, the Firefox add-on ID and the extension's message
    names.

  Renaming these would orphan existing meetings, keys and the extension's pairing. The
  fallback device name is now "Koalino device".
- **License:** GPL-3.0-or-later (`LICENSE`, `Cargo.toml`, the two `package.json`). The owner first
  asked for GPLv2. Thirteen Rust crates are Apache-2.0 only (ring, prost for libp2p, aws-lc-sys
  and others), and Apache-2.0 is incompatible with GPLv2 but compatible with GPLv3.
- **Notices:** `THIRD_PARTY_NOTICES.md` is generated by `tools/licenses/third_party_notices.py`;
  every listed license is GPLv3-compatible. The app shows the GPL notice and the Flutter package
  licenses under Settings → About Koalino, as GPLv3 §0 asks of interactive programs.
- **Open:** Qualcomm's QNN libraries (#75, phone NPU) are proprietary and need a review before
  they ship.

## 2026-11-12 — #114 Meetings go to the owner's own AI app; the answer comes back as the summary

- **Why:** the owner uses their own Claude and ChatGPT accounts and does not want to share
  them. Koalino never holds their credentials or Claude.ai OAuth tokens (Anthropic's terms).
  Instead, the meeting is handed to the official app on the phone, signed in as the owner.
- **Hand-off (Android):** the core writes the meeting as Markdown (title, speakers, transcript,
  notes, without any earlier summary) plus its photos and files into `cache/shared/handoff`.
  It keeps at most 20 files, 50 MiB each and 100 MiB in all. The folder is emptied before each
  hand-off and at start-up. `SharedFiles.kt` sends them with the filled-in prompt (ACTION_SEND or
  SEND_MULTIPLE) to `com.anthropic.claude` or `com.openai.chatgpt` when installed, else through
  the share sheet. The prompt is also put on the clipboard, for apps that drop shared text.
- **The answer:** Android gives an app no other app's reply. So a summary hand-off waits up to
  an hour, and the answer can come back in two ways:
  - **Copied:** when Koalino returns to the foreground, new clipboard text of at least 80
    characters is kept;
  - **Shared back:** a text share arriving meanwhile offers 'Save as the summary of "<title>"'.

  The answer is stored as an edited summary (`keep_answer`), recording the app as its source and
  the prompt used. Undo puts back the earlier summary (`restore_summary`). A chat hand-off leaves
  nothing pending: the conversation stays in the other app, as the owner asked.
- **Export Share:** the Export menu's "Share as Markdown / PDF / JSON" sends the export with its
  attachments to any app. Markdown attachments are flattened beside the file, because the
  provider serves only that folder's own files, so the file's relative links no longer resolve.
- **Not covered:** desktops (there is no comparable app hand-off; the API-key connections remain),
  and a live test with Claude or ChatGPT. That test would send a meeting to the cloud, so it
  awaits the owner. Package visibility and both apps' acceptance of SEND and SEND_MULTIPLE of
  `*/*` were checked on the S23.

## 2026-11-12 — #109 The device times a public clip to confirm Automatic's model

- **What runs:** when Automatic is on and no recording has measured its model on this device, the
  after-meeting worker transcribes a 15 s clip eight times with that model, in efficient mode when
  Automatic chose it. Eight runs give the 120 s and five chunks the #68 rule needs.
- **Telemetry:** each run is saved as an ordinary transcription row. So the #68 proposal ("falling
  behind: try a smaller model") works from the first day instead of after two minutes of
  recordings. When the model falls behind (above 0.9× real time), the next smaller model on the
  device is timed too, until one keeps up. Nothing downloads, and Automatic itself does not change:
  the owner sees the proposal, as with any measurement.
- **When:** only while a pass could run: no recording, and on a phone only while charging. It stops
  between runs as soon as that changes, keeping no partial result. It needs the after-meeting pass
  to be on, because it runs on that worker. When a check is due, the worker starts at launch on a
  desktop, and on a phone once it charges. A charging report that came before the core opened is
  now repeated once it opens; before, it waited for the next battery broadcast, which also delayed
  leftover speech's pass. Results are kept in `<database>.speed-check.json` with
  a key of the device profile and the models on the device, so new hardware or a new model checks
  again.
- **Clip:** 15 s of Italian from the VoxPopuli test set (CC0, European Parliament), compiled into
  the core (480 KB). It is never played. Short chunks match live recording, whose VAD chunks are at
  most 15 s.
- **Settings:** the Automatic row adds "Checked on this device: 0.3× real time, so it keeps up with
  recordings." If the model is too slow, it adds which smaller model keeps up, or that none does.

## 2026-11-13 — #108 The computer's sound is recorded as a second track

- **What:** on a desktop, a recording also captures what the computer plays, such as the other side
  of a call, beside the microphone. It is transcribed as its own track: its segments are marked
  `computer_sound`, in the meeting document and in SQLite. The after-meeting passes keep those
  segments, because only the microphone's speech is retained and re-transcribed. On by default;
  Settings can turn it off ("Also record the computer's sound"). Phones cannot: Android needs a
  screen-capture grant to hear other apps.
- **Capture:** `cpal`, already used for the microphone, behind a `computer-sound` feature:
  - Windows: WASAPI loopback of the default output.
  - Linux: the default sink's `.monitor` source, through cpal's pure-Rust PulseAudio host, which
    also serves PipeWire.
  - Yogurt's own libpulse code was not taken, to avoid a new system library.
  - The microphone stays on ALSA on Linux, as before.
  - New crates: `pulseaudio` and `enum-primitive-derive`, both MIT.
- **Timing:** both tracks are drained on the same 100 ms tick. The computer's samples are placed so
  they end at the microphone's captured time. That covers a loopback that opens late, and one that
  sends nothing while nothing plays: the track jumps ahead when it falls more than 0.2 s behind.
  That keeps the two tracks within the echo search's 0–300 ms.
- **Echo without headphones:** the microphone also hears the speakers, so the other side's words
  land on both tracks. Ported from Yogurt (MIT, credited in `THIRD_PARTY_NOTICES.md`), without its
  per-line audio check:
  - Detection compares 10 ms loudness envelopes over 2 s windows at lags of 0–300 ms. Echo counts as
    heard when at least 40% of the windows where both carry sound correlate at a steady lag, or
    55% at any lag.
  - Two people talking over each other correlate rarely and at random lags, so they do not count.
  - Once echo is heard, a microphone line is dropped when its words copy a computer-sound line near
    the same time, in an echoing window.
  - English and Italian short replies ("sì", "ok", "esatto") are never dropped.
  - The meeting stores `echo_heard`, so the after-meeting passes, whose audio is gone, drop the
    copies from their new text too.
- **Checked on deb-gui (PipeWire 1.4):** a recording opens two capture streams: the microphone
  through ALSA, and the default sink's monitor through `cpal-pulseaudio`. Pause closes both and
  resume reopens both; stop releases them. On each close, the `pulseaudio` crate logs "Reactor
  error: Client disconnected"; that is its client ending, not a failure.
- **Not verified:** the content of a real capture. Testing it needs sound played in a VM, which
  awaits the owner's permission.
- **Also:** `THIRD_PARTY_NOTICES.md` now lists the crates of every feature (`--all-features`). cpal,
  ort, whisper-rs and openssl had been missing; all of their licenses are GPLv3-compatible.

## 2026-11-13 — #112 A read-only MCP server lets the owner's own assistant read the notes

- **What:** `note_taker_mcp`, a small binary shipped beside the desktop app, speaks the Model
  Context Protocol over stdio. Its six tools only read: list and read meetings, search, list and
  read projects, and read a project's text files. Nothing can be written through it, and it opens
  no network listener.
- **Separate binary, not a CLI subcommand:** the desktop bundles ship only the core library, and
  the CLI composes recording and the AI pipeline. The server is built without AI features, so it
  is small and quick to build, and Settings can point to it beside the app's executable.
- **No new crates:** JSON-RPC 2.0 with `serde_json`, line by line on stdin and stdout. An MCP SDK
  would have added a framework and an async runtime for six tools.
- **Off until turned on**, in Settings → AI assistant access (`<db>.assistant-access.json`). The
  page warns that the assistant may send what it reads to its online model, under the owner's own
  account. The switch is read at every call, so turning it off stops the next read. While it is
  off, each call answers how to turn it on.
- **Keys stay put:** the server unlocks the database as the app does (the platform keychain). The
  snippets Settings shows hold only the binary's path and the database's path, never a key or a
  passphrase. The server refuses a database path that does not exist, so a wrong path cannot
  create an empty database with a new key.
- **Logs** name the tool and whether it worked, never arguments or results. Standard output
  carries only protocol messages.
- **Checked:** unit tests for the exchange, the tools and the switch; the binary answering over
  stdio on a scratch database. Not tried with a live Claude session, to keep meeting content
  away from a cloud model without the owner's go-ahead.

## 2026-11-13 — #111 Offer to record a call, and stop when it ends

- **How a call is noticed:** the desktop asks the system which apps capture the microphone every
  3 s.
  - Linux: `pactl -f json`, for PulseAudio or PipeWire's pulse server. Paused (corked) streams and
    streams on monitor sources do not count.
  - Windows: `reg query` on the microphone consent store.
  - Running the standard tools needs no audio or registry crate, and one look every few seconds is
    cheap.
- **What counts:** a known call app (Zoom, Teams, Webex, Slack, Discord, Skype, Signal and
  others) or a browser, since Meet, Jitsi and Teams run in browsers. The app's own process never
  counts. A call starts after 3 s and ends after 20 s without a call app, so muting or switching
  devices does not end it.
- **Never records by itself:** the app shows "`<app>` is using the microphone. Record this call?"
  above every screen. Only Record starts a recording, titled "`<app>` call".
- **Auto-stop** applies only to a recording started from the offer, and it can be turned off. A
  recording the owner started is never stopped by a call. Both switches are in Settings and on by
  default.
- **Checked on deb-gui:** a silent `parecord` stream named "zoom" brought the offer, and Record
  started "Zoom call". Ending that stream stopped the recording 20 s later and released every
  capture stream. Nothing was played.
- **System notification (Linux):** `notify-send` with Record and Not now buttons, run by the app,
  with no plugin. It is withdrawn when the offer is answered in the app.
- **Child processes:** while the Flutter engine has a process of its own running (that
  notification), it reaps children with `waitpid(-1)` and can collect the status of the core's
  `pactl`, or of Claude Code / Codex (#105). The core then keeps the output it read, and a look
  that failed once no longer stops the watch. deb-gui showed the bug: the call's end went
  unnoticed after a Record tap in the notification. After the fix, it stopped as it should.
- **Not yet:**
  - a system notification on Windows;
  - Windows on a VM: win11's snapshot needs a missing `virtio-win.iso`, so the consent-store parser
    is tested only on the documented output;
  - Android.

## 2026-11-13 — #111 Windows: notice calls from the audio sessions

- **Problem:** on win11 (Windows 11 Home 26H2, build 26300) the microphone consent store has no
  `LastUsedTimeStart`/`LastUsedTimeStop` values any more; usage moved to a database under
  `C:\ProgramData\Microsoft\Windows\CapabilityAccessManager`. With Edge holding the microphone,
  `reg query` saw nothing and no offer appeared.
- **Fix:** the core also lists the active WASAPI capture sessions (`IMMDeviceEnumerator` →
  `IAudioSessionManager2` → active, non-system sessions → process image name). The process ID comes
  along, so the app's own recording is left out. Store apps show as their own process. `reg` is
  now only the fallback when the sessions cannot be read (logged once): on the single-core win11
  VM, spawning it took 10.6 s per look against 4 ms for the sessions, so the busy app managed one
  look a minute, too few for the 3 s it takes to notice a call.
- **Dependencies:** none new. The `windows` crate approved for UI Automation (#113) gains the
  `Win32_Media_Audio`, `Win32_System_Threading` and COM/variant features.
- **Banner bug:** `NoCallNotifier` (Windows) answered `dismissed` at once, so the controller
  withdrew the offer the moment it appeared. A missing `notify-send` on Linux did the same. A new
  answer, `unavailable`, now leaves the banner up.
- **Log:** the core logs at info when a call app takes or leaves the microphone, naming only the
  app kind (`Edge`, `Teams`), never a window or person.
- **Checked on win11:** a page in Edge holding the microphone. The app showed "Edge is using the
  microphone. Record this call?", and Record started "Edge call", which saved when stopped. The
  auto-stop was not run there: Edge's other tabs are the owner's own. It is the same code as on
  Linux. Audio sessions are per logon session, so a look from SSH sees none of the desktop's
  apps.

## 2026-11-13 — #116 follow-up: a clip for voices only the final pass found

- The final speaker pass can find a voice the live pass never clustered. Such a voice had no
  review clip, so the owner could only name it from its words.
- The after-meeting pass (#87) still holds the meeting's speech. It now cuts a clip for each
  unknown voice without one: the voice's longest microphone segment inside one retained chunk, up
  to 15 s, and none under 1 s, as live clips.
- The clip is raw audio until the voice is named or the meeting is deleted, like every review
  clip. Nothing was played to check it; a unit test checks which voice gets a clip and which
  audio it holds.
- The combined pass of a meeting recorded on several devices (#88) does the same on the first
  recording's timeline. A clip in any of the meeting's recordings counts, so no voice gets two.

## 2026-11-13 — #113 Speaker names read from the call window

- **Context:** call apps show who is speaking. A name on screen while an unknown voice speaks is
  strong evidence for naming that voice. The owner approved new dependencies for this:
  - `zbus` on Linux, already built for `keyring`;
  - the `windows` crate's UI Automation on Windows, the same release `cpal` builds.
- **Reading:** while a recording captures audio, a `speaker-name-watcher` thread asks the platform
  reader every 2 s.
  - Linux reads AT-SPI over D-Bus; Windows uses UI Automation.
  - Only windows whose title is a call app's or a meeting site's are walked (Teams, Meet, Zoom,
    Jitsi, Webex, Whereby, Discord, Slack huddles, Skype, BigBlueButton, GoToMeeting).
  - Each reading is limited to 3,000 elements and 800 ms, and each D-Bus call to 300 ms.
  - The reader only reads: nothing is clicked or focused, and labels and names are never logged.
  - A label gives a name when it says that person is speaking ("Alice is speaking", "Talking:
    Alice", "Alice, video is off, speaking", "Alice (speaking)", "Alice sta parlando").
  - Text that is not a name is refused: more than 5 words or 60 characters, or words such as
    "you" and "someone".
- **Storage:** each reading covers ±1 s, and readings of one name less than 3 s apart join into
  one span. At stop the spans are attached to the meeting as a sealed `speaker-names-…vtt`, with
  one `<v Name>` cue per span. The attachment syncs like the Teams captions (#102), which use the
  same format.
- **Suggestions, not assignments:** the voice gallery reads a meeting's `speaker-names`, Teams
  captions and Teams live-transcript attachments. It compares their spans with each unknown voice's
  segments.
  - A voice needs at least 2 s of speech under named spans.
  - One name must cover at least 60% of that speech.
  - The Teams transcript published after the meeting is not used, because it is timed from the
    Teams meeting's start, not the recording's.
  - The app and web fill the name field with the suggestion and say where it came from. Only Assign
    names the voice.
- **Accessibility on Linux:** browsers and Electron apps expose their pages only while assistive
  technology is active.
  - While "Read speaker names from the call window" is on (Settings, on by default), the app sets
    `org.a11y.Status.IsEnabled`. The bus launcher keeps that in the desktop settings.
  - When it had to turn the flag on, the app asks for a restart of the browser and call apps. On
    deb-gui, Firefox ESR started before the change did not expose its page; after a restart it did.
- **Verified on deb-gui:** `cli call-names` read a local fixture page in Firefox ESR, with no
  audio. All three label shapes were read: a tile label, a comma list with "speaking", and a
  "Talking: …" text line.
- **Not verified:**
  - the labels of real Teams, Zoom or Meet calls, which were not captured, so their phrasing may
    need new patterns;
  - a suggestion end to end, which needs speech, and playing audio needs the owner's permission.
    It is covered by unit tests.

## 2026-11-13 — #108 Checked with sound: the computer's track was 2 s late

With playback allowed, deb-gui played public clips: the other side into a silent virtual
speaker (the default sink, whose monitor the app records) and the owner into a virtual microphone.
- **Headphones (no leak):** every turn of the other side was a `computer_sound` segment, and the
  microphone's turns were attributed to the owner's saved voice.
- **Echo (the speaker leaking into the microphone at half volume, 60 ms late):** nothing was
  dropped; each of their lines appeared twice. Saving what the detector received showed why:
  - The microphone's chunks matched the source at a 3.14 s offset, the computer's at 5.08 s.
    The computer's sound was 2 s late on the timeline.
  - Without a buffer size, PulseAudio delivers a record stream in fragments of about 2 s. The
    recorder places each batch to end at the microphone's time, so every batch landed 2 s late.
  - The detector looks 0–300 ms apart, so it never saw the leak.
- **Fixes:**
  - The monitor stream asks for 50 ms blocks (`BufferSize::Fixed`). Not smaller, because cpal
    also sets the server's queue limit to that size.
  - Afterwards every active window correlated at r = 0.99, but at −60 ms: the microphone, on
    ALSA, still lands about 120 ms before the computer's sound. The detector now searches
    −300 to +300 ms. Two people talking over each other is still not echo in its tests.
- **Windows (win11):** the WASAPI loopback, left at its default period, recorded a clip played on
  the speakers. With the chosen `tiny-q5_1` model, both voices were transcribed as the computer's
  sound.
  - cpal reported each WASAPI data discontinuity as an overrun, on both streams: 307 warnings in
    3 minutes. The VM runs Windows on one usable vCPU, and any Flutter app keeps it busy there.
  - Each capture stream now logs its first error by name, then at most one line a minute with the
    count held back (`CaptureErrorThrottle`).

## 2026-11-13 — #113 on Windows, and covered call windows

- **Windows verified:** on win11, `cli.exe call-names`, run in the desktop session, read the same
  fixture page in Edge through UI Automation. It followed every change of speaker within a second,
  for all three label shapes.
- **Covered windows freeze:** while a console window covered Edge completely, both our reader and a
  .NET UI Automation client saw the same tree for the whole run. Chromium (Edge, Chrome, and the
  WebView2 inside new Teams) stops rendering a fully covered or minimized window. Its accessibility
  tree is updated during rendering, so it keeps the last "is speaking" label. A frozen label
  cannot be told apart from one person talking for a long time.
- **Guard:** `suggest_names` now suggests a name only when it wins for exactly one voice. A frozen
  label spans every voice, so it would otherwise be suggested to several of them.
  - The cost: one person whose voice was split into two clusters gets no suggestion for either.
    The owner then names them by hand, as before. A missing suggestion is cheaper than a wrong
    one.
- **For the owner:** keep the call window at least partly visible while recording. A small window
  beside the notes is enough; a minimized one is not.

## 2026-11-13 — #87 The after-meeting pass's model in Settings, and the pass on the web

- The pass's own model was already a saved setting (`transcript_pass_model`), but nothing set it.
  Settings now shows "Model for this pass" while the pass is on: "Same as for recording", or a
  model already on the device. A larger model gains most here, because the pass has time after
  the meeting (on ponte, `small` 32% → turbo 15% difference).
  - Only downloaded models are offered. Downloading stays in the model list above, so the choice
    never starts a download.
  - A model removed later falls back to the recording model: the settings read back `null`.
- The web Settings page gains the switch and the menu (`/api/settings/after-meeting-pass`), for
  the daemon on a server or desktop. Its menu lists the models on the daemon's machine.

## 2026-11-13 — #117 No API keys: OpenAI transcription and keyed AI connections removed

- **Why:** the owner uses Koalino for themselves with their Claude and ChatGPT plans, through
  Claude Code and Codex (#105), and asked to remove the API key features. Keys were an extra
  secret to store, an extra way for meeting content to leave the device, and a source of
  confusing errors (`missing_key`, `keychain_failed`).
- **Transcription:** always on the device (whisper.cpp). The OpenAI Whisper adapter, the cloud
  fallback, the provider choice, the key field and its FFI calls are gone, as are
  `NOTE_TAKER_TRANSCRIPTION_PROVIDER`, `OPENAI_API_KEY`, `NOTE_TAKER_OPENAI_WHISPER_MODEL` and
  `NOTE_TAKER_OPENAI_LANGUAGE`. A saved `openai` choice is ignored and dropped at the next save.
  `TaskType::CloudTranscription` and `TranscriptionModel::ThirdParty` stay, so old telemetry rows
  and older peers still read.
- **AI connections:** an OpenAI-compatible server on this device or the local network (Ollama,
  LM Studio, llama.cpp, vLLM), or Claude Code / Codex. No authorization header is ever sent. The
  `anthropic` kind and the OpenAI, Anthropic and Mistral presets are gone; a saved Anthropic
  connection is dropped when the settings are read, and a sent `api_key` is ignored.
  `llm_unauthorized` now says to use a server that needs no key.
- **Old keys:** sealed key files an older Android version wrote (`llm-*.sealed`,
  `openai_key.sealed`) are deleted when the AI services open. Keys an older desktop version put
  in the platform keychain (`note-taker-openai`, `note-taker-llm-<id>`) are left alone: reading the
  keychain at start-up can ask to unlock it. The owner can delete them by hand.
- Claude Code and Codex still run with `OPENAI_API_KEY` / `ANTHROPIC_API_KEY` removed from their
  environment, so the plan's sign-in answers, not a stray key.

## 2026-11-14 — #111 Calls noticed on Android, and a Windows toast

- **Android:** an ordinary app cannot see which app captures the microphone (the recording
  configurations hide the client's package). Kotlin therefore reports a call while the audio mode
  is `MODE_IN_COMMUNICATION` (set by VoIP apps for a call) or a capture uses the
  `VOICE_COMMUNICATION` source, as one unnamed user that Dart sends with each poll. The core keeps
  the start and end logic, names it "A call app", and the recording is titled "Call". Koalino's
  own recorder uses neither, so it never counts.
  - The offer is a high-importance notification. Record opens the app, because a microphone
    foreground service can start only while the app is visible.
  - Verified on the S23 by holding the communication mode from a shell helper for 25 s, with no
    microphone and no sound: notification and banner shown, both withdrawn after the grace period.
    Record from the phone's notification was not tried, since it would record the owner's room.
  - Detection runs while the app's process lives; Android may stop a backgrounded app.
- **Windows toast:** Windows PowerShell 5.1 cannot subscribe to WinRT events, so a script could
  show a toast but not learn which button was pressed. The core shows it instead
  (`adapters/notification/call_toast.rs`, existing `windows` crate with three more features) and
  receives the in-process Activated/Dismissed events; the AppUserModelID is registered under HKCU,
  no shortcut needed. Verified on win11: the toast from "Koalino" offered "Edge", Record in it
  started and saved "Edge call", and dismissing it cleared the banner. On the VM the pop-up was not
  always visible over the app's window, but the toast was always in the notification centre.

## 2026-11-14 — #75 Whisper on the phone's NPU, downloaded on request

- **Engine:** Qualcomm AI Hub's precompiled `whisper_small_quantized` (w8a16, SM8550) on
  Microsoft's ONNX Runtime 1.29.0 with the QNN execution provider, CPU fallback off. The spike
  showed it on the S23's Hexagon NPU at about 0.05× real time, with a word error rate of 8.6%
  against 8.0% for unquantized Whisper Small on public FLEURS Italian clips.
  - The core does the rest in Rust with no new library: log-mel (matches faster-whisper to
    1e-5), the vocabulary and mel filters read from the shipped `ggml-base.bin`, and greedy
    decoding.
  - No word timing, glossary prompt or previous-text context yet; the after-meeting pass still
    uses whisper.cpp.
- **Licensing (owner's choice: download on demand):** the QNN runtime is under Qualcomm's AI Stack
  License, which is not GPLv3-compatible. So the APK carries none of it.
  - Settings → Transcription model → "Transcribe on the NPU" shows a dialog naming Qualcomm's
    terms. After "Accept and download", `NpuDownload.kt` fetches the QNN runtime AAR (Maven
    Central) and the model zip (AI Hub's public bucket), each pinned by size and SHA-256.
  - Only the needed files are extracted, into a staging folder, and moved into place when both
    archives are complete.
  - ONNX Runtime itself (MIT) ships in the APK.
  - `THIRD_PARTY_NOTICES.md` lists ONNX Runtime and, separately, what is downloaded only on
    request.
- **Use:** the setting `npu_whisper` in `<db>.transcription-settings.json`, off by default. It is
  turned on when the download ends. The pipeline tries the NPU first when the setting is on, the
  files are complete and the shipped multilingual model exists. If the NPU will not open, it
  falls back to whisper.cpp with a warning. The section only shows on a chip that runs the model
  (`ro.soc.model` = SM8550).
- **Android details found on the device:**
  - An app reaches the NPU only if its manifest declares `libcdsprpc.so` with
    `<uses-native-library>`. The shell did not need it; the app failed with
    `QNN_DEVICE_ERROR_INVALID_CONFIG`.
  - `ADSP_LIBRARY_PATH` is set to the downloaded `qnn` folder in `MainActivity.onCreate`.
  - Loading the libraries from the app's files folder works; `LD_LIBRARY_PATH` is not needed.
- **Verified on the S23:**
  - The on-device test transcribed JFK correctly in 0.56 s after a 2.3 s load.
  - In the app, the download, checks and installation finished within a minute on Wi-Fi, and the
    switch turned on.
  - After a restart, the app opened the NPU session (the HTP skeleton loaded from the downloaded
    folder, QoS raised).
  - A recording transcribed in the app on the NPU was not tried: the phone is not to record.

## 2026-11-14 — #75 NPU Whisper: timing, prompt, silence, and the after-meeting pass

- **Timestamps:** the NPU decoder now uses Whisper's timestamp rules (a window starts with a
  timestamp no later than 1 s, timestamps come in pairs and never go back, and a timestamp is
  taken when timestamps together are likelier than any text token).
  - Segments come from the timestamp pairs. The next 30 s window starts after the last finished
    segment, so a word cut at the window's edge is decoded again.
  - Word times are approximate: the precompiled model gives no attention weights for whisper.cpp's
    alignment, so each segment's time is shared among its tokens by byte length.
  - Confidence is each word's least sure token, from the dequantized logits.
- **Prompt:** the chunk's previous text and the glossary, then the text decoded so far, at most 64
  tokens (the decoder holds 200 positions).
  - It is tokenized as whisper.cpp does: the ggml file has no BPE merges, so no tokenizer is added.
  - On 83 s of public FLEURS Italian clips, the glossary did not fix the terms given and moved
    the error rate by about two words either way. It stays for consistency with whisper.cpp; its
    benefit on the NPU is not shown.
- **Silence:** Whisper's no-speech rule drops a window when `<|nospeech|>` was likelier than 0.6
  and its text tokens' mean log probability is below −1. The mean is over text tokens only, since
  near-certain timestamps would dilute it.
  - On the same clip it removed a phrase made up in the trailing silence: WER 8.2% → 3.4%,
    83 s transcribed in 6.2 s on the S23.
- **After-meeting pass:** with the NPU on and no separate pass model chosen, the pass uses the NPU,
  since "the live model" is then the NPU's. It falls back to whisper.cpp with the live model. A
  model chosen for the pass keeps whisper.cpp.

## 2026-11-14 — #75 NPU prompts tokenized exactly as Whisper does

- **Problem:** the ggml file has the vocabulary but no BPE merge list, so prompts were tokenized
  by longest match (as whisper.cpp does), which can differ from what Whisper was trained on.
- **Decision:** infer the merges from the vocabulary. In Whisper's tiktoken vocabulary a token's
  ID is its merge rank, so byte-pair encoding repeatedly joins the adjacent pair whose joined
  bytes form the known token with the lowest ID. No data file or tokenizer library is added.
- **Validation:** 697 lines (the clip's FLEURS references with and without a leading space, plus
  edge cases) against Hugging Face's `tokenizers` with the `openai/whisper-small` tokenizer.
  - 697/697 identical once the HF file's missing first merge (`Ġ t`, a known conversion bug of
    that file) is restored; every earlier difference came from that merge.
  - So the inferred tokenizer matches OpenAI's own (`multilingual.tiktoken`).
- **Effect:** on the 83 s FLEURS clip, the WER was 3.4% without a prompt, 4.8% with
  "imbarcazioni, missioni.", 3.4% with a five-term glossary and 4.1% with unrelated names. Same as
  with the longest-match tokenizer: the glossary still does not fix "embarcazioni" or "emissioni"
  on the quantized NPU model. The tokenizer stays because it is what Whisper expects.
