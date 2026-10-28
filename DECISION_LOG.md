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
