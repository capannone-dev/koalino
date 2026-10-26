<!-- Captures the current implementation boundary for contributors resuming work on the project. -->

# Engineering context

Issues #1 and #2 establish the Rust core and its first storage adapter. `core/domain` is pure business code; `core/ports` provides interfaces; `core/application` coordinates ports; `core/adapters` contains infrastructure. `core/adapters/storage/sqlite_adapter.rs` implements the `NoteRepository` port with bundled SQLite and stores serialized Automerge documents as opaque BLOBs. `core/application/meeting_crdt.rs` creates, edits, loads, and merges those documents without exposing Automerge across a port boundary. AI, libp2p, Flutter, and server adapters remain unimplemented.

Logging emits JSON to stdout and a daily `app_log` rolling file. The `LoggingRuntime` provides a reloadable filter for a future UI or CLI runtime-control surface. Events must include `device_id` and must never log raw audio, voice embeddings, E2EE keys, or other sensitive values.

The SQLite adapter logs initialization and completed persistence operations with device and meeting identifiers only. It stores creation time and acquisition-device metadata, rejects orphan writes, and does not yet store voice embeddings; a future implementation must encrypt voice embeddings at rest with AES-256-GCM.

Issue #3 adds the portable `AudioCapture` port, `AudioConfig`, `AudioChunk`, and `AudioState` domain values. `core/adapters/audio/local_recorder.rs` is a dependency-free local state machine with terminal `Stopped` sessions: it routes signed 16-bit PCM chunks only in `Recording`, publishes state transitions through standard-library receivers, rejects invalid lifecycle commands with `AudioError::InvalidState`, and logs only state transitions. Its `push_test_chunk` seam is intentionally adapter-local; no application layer depends on it. A future P2P streaming adapter can implement the same port without changing application consumers. OS permissions, background services, wake-locks, and lock-screen controls remain Flutter responsibilities.

Issue #4 adds `DeviceManifest` and `TranscriptionModel` domain values plus the application-level `ComputeDispatcher`. The dispatcher owns only injected `TranscriptionEngine` trait objects, so local Whisper, third-party API, and future encrypted remote-proxy adapters remain interchangeable. Manifests contain only a device ID, CPU core count, GPU VRAM, and advertised model names. `Automatic` routing filters by model compatibility and picks the highest GPU-VRAM-then-CPU score; explicit local and peer choices are honored. Unit tests use a deterministic mock engine and never initialize or download AI models. Manifest transport/broadcast awaits the assigned P2P work package.

Issue #5 adds the Flutter package under `ui` and a deliberately narrow Rust `cdylib` boundary. The
Dart `RecordingController` depends on `RecordingBridge`, while `RustRecordingBridge` alone uses
`dart:ffi` to call `note_taker_start_recording` and `note_taker_set_log_level`. Widget tests inject
a fake bridge, proving widgets do not load or access Rust structs. The C ABI returns primitive
status codes and keeps the process-local `LocalRecorder` and logging runtime on the Rust side.

Issue #6 adds a libp2p-only network adapter. It generates an ephemeral Ed25519 identity and builds
a Tokio swarm with TCP, Noise encryption, Yamux multiplexing, and mDNS local discovery. The adapter
translates mDNS peer identifiers into portable `NetworkEvent` values and implements `P2pNetwork`
and `PeerDiscovery`; application code records reachability solely through those events. Unit tests
use a mocked `P2pNetwork` and never instantiate the adapter, bind an address, or open TCP/UDP
sockets. Payload transmission intentionally remains rejected until a future encrypted application
protocol (for CRDT sync or manifests) is defined.

The current sync application work adds a versioned `SyncMessage` envelope around Automerge sync
bytes plus `SyncManager`, scoped to a single existing meeting document. It starts a peer-specific
handshake on discovery, validates received envelopes, applies Automerge changes through
`MeetingCrdtService`, and persists the result through `NoteRepository`. A local edit calls
`notify_local_change()` to produce a fresh sync message for encrypted broadcast. Unit tests use
mocked ports only. Issue #8 configures the adapter-local `/note-taker/sync-message/1` libp2p
request-response protocol with bounded length-prefixed opaque payloads. The adapter sends only on
authenticated connections and translates inbound requests to `NetworkEvent::MessageReceived`; no
libp2p type crosses the application boundary. Its memory-only integration test proves directed and
broadcast payloads traverse Noise and trigger the portable event without binding TCP or UDP.

Issue #9 adds `SpeakerIdentity`, UUID-keyed `VoiceEmbedding` variants, and `VoiceMatchResult`.
`VoiceIdentityManager` relies only on the `VoiceVectorDB` port: it never computes cosine
similarity. It classifies adapter-returned scores as strong above 0.80, weak from 0.50 through 0.80
inclusive, and no match below 0.50. User confirmation of a weak match adds a fresh variant UUID to
the selected identity. `InMemoryVoiceVectorDB` is a dependency-light cosine-search adapter for
tests and development; it is not persistent or encrypted and must be replaced before production
biometric storage.

Issue #10 replaces the earlier diarization placeholder with an async `DiarizationEngine` port that
returns time-bounded `SpeakerSegment` values with derived embeddings. `DiarizationProcessor` uses
only that port, `VoiceIdentityMatcher`, and `NoteRepository`: strong matches are returned as
identity links, while weak and absent matches create UUID-keyed `UnassignedVoiceClip` records for
manual review. Unit tests use hardcoded diarization segments and mocked matcher/repository ports;
no ML model or dependency is initialized. Temporary clip bytes and embeddings are never logged.

Issue #11 adds the Voice Gallery FFI/UI boundary. `UnassignedVoiceClip` now retains the derived
segment embedding and optional weak candidate so confirmation can be deferred until user review;
these sensitive values stay in Rust/SQLite. `note_taker_get_unassigned_clips` materializes each
temporary clip in a configured local review directory and returns JSON metadata plus its absolute
file path. Flutter uses `audioplayers` to open that path directly. `VoiceGalleryController` is a
Riverpod-provided, bridge-injected controller; its widget tests use a fake bridge and do not load
the native library. Confirmation and creation commands call the existing `VoiceIdentityManager`,
not Flutter-side matching logic.

Issue #12 adds privacy-safe AI telemetry. `TaskMetrics`, `DeviceLoadStats`, and `AggregatedStats`
are domain values persisted through the `MetricsRepository` port. `ComputeDispatcher` measures the
selected transcription engine call, then saves duration/latency, a coarse CPU-or-GPU power estimate,
and timestamp only after success. `SqliteNoteRepository` stores metrics and current device load in
dedicated tables and computes total duration plus average latency. Tests use its in-memory SQLite
constructor; no AI model is loaded and no input/output payload is persisted as telemetry.

Issue #15 adds provider-neutral external meeting processing. `MeetingBridge` emits portable
isolated `ExternalMediaChunk` and `SlideCapture` values; it does not expose Microsoft/Azure types.
`TeamsIdentityMapper` uses stable `external_id` mappings and creates an unseen local
`SpeakerIdentity` with the external `display_name`, while `TeamsProcessor` auto-stores a fresh
voice variant and forwards audio through `TranscriptionEngine`. Guaranteed participant streams
bypass the local weak-match/diarization workflow. All core tests use standard-library channel and
repository mocks, with no external bridge connection.

Issue #16 adds recoverable OS interruption handling for local audio. `LocalRecorder` transitions
from Recording to Interrupted, reports `AudioError::Interrupted`, emits a safe warning, and resumes
only through Start; Stopped remains terminal. The FFI runtime retains its recorder behind a
process-wide `OnceLock<Mutex<...>>` and now exposes background-safe Start/Pause/Stop/interruption
commands plus distinct state-query codes. Android, Windows, and headless Linux integration remains
documentation-only; no platform service implementation or external OS API is coupled to Rust.

Issue #13 adds attachment domain values plus ports for metadata extraction and attachment
persistence. `AttachmentManager` owns only injected `MetadataExtractor` and `NoteRepository`
ports: it receives a UTC capture time, gets overlapping `MeetingWindow` values, and ranks inclusive
windows by linear midpoint proximity (1.0 at the midpoint, 0.0 at either edge). It does not read
EXIF/PDF metadata or access files itself. Unit tests use a hardcoded mock extractor and mock
repository windows only; no real file metadata is accessed in CI. Existing meeting IDs remain
typed `MeetingId` strings across the established storage boundary, while attachments themselves
have UUID identifiers.

Issue #14 adds the attachment import FFI/UI flow. `note_taker_process_attachment` composes the
filesystem timestamp adapter with the existing `AttachmentManager` and returns caller-freed JSON
metadata only; `note_taker_confirm_attachment_link` persists a selected link through the SQLite
`AttachmentRepository`. Flutter's Riverpod `AttachmentController` parses that JSON itself, and
the Material 3 paperclip workflow keeps platform file selection behind an injectable picker. Files
remain local: FFI transports only a path, metadata JSON, and opaque IDs; file bytes are never
transferred or stored in SQLite.

Issue #17 adds the headless Axum adapter. `ServerAppState` accepts only shared repository-port and
voice-vector-port implementations; handlers translate safe domain data into REST DTOs and never
issue SQL. The WebSocket event bus carries opaque meeting/clip identifiers and sync status only.
Integration tests use in-memory SQLite plus Axum `oneshot`; their WebSocket test binds an OS-chosen
loopback port, avoiding CI collisions.

Issue #18 adds the clap-based Linux CLI driving adapter. Its parser is library-tested separately
from the executable; `CoreCliService` delegates recording to `AudioCapture`, meeting/telemetry data
to repository ports, and manual clip resolution to `VoiceIdentityManager`. The binary opens the
same SQLite path as the daemon and filters logs to error so terminal output remains human-readable.

Issue #19 adds the Next.js App Router dashboard in `web/`. Typed clients consume the local Axum
meeting, telemetry, voice-gallery, assignment, and WebSocket contracts; components never read
SQLite. Node verification is intentionally delegated to `web-ci.yml` because the coding sandbox
lacks Node. The new assignment API resolves clips through `NoteRepository` and invokes
`VoiceIdentityManager`, keeping raw audio and embeddings outside HTTP responses.

Issue #20 adds an opt-in `ai/local_whisper_cpp.rs` adapter using the optional `whisper-rs`
whisper.cpp binding. The default Cargo feature set has no native model dependency, so `make
rust-test` continues to use only mocks. `LocalWhisperCppTranscriptionEngine` owns both native
context and an adapter-local source of normalized 16 kHz mono PCM keyed by `AudioRecording`; it
implements only `TranscriptionEngine` and returns portable `Transcript`/`TranscriptionError`
values. A feature-gated contract test runs real local inference only when both `local-ai` and
`NOTE_TAKER_WHISPER_MODEL` are explicitly set; no model is downloaded or initialized by default.

Issue #21 adds a separate `/note-taker/file-transfer/1` behaviour alongside CRDT synchronization.
`FileSyncManager` uses only file-network and content-addressed-storage ports, keeps an expiring
peer availability registry, and queues best-effort attachment fetches after CRDT metadata is
persisted. Explicit 64 KiB frames are SHA-256 verified before atomic CAS commit; integration tests
use Noise over libp2p memory transport only.

Issue #22 adds the default-build OpenAI Whisper adapter with a reqwest multipart boundary and
adapter-owned WAV source. It implements only `TranscriptionEngine`; API keys are loaded from
`OPENAI_API_KEY` and are never logged. `AiConfig` and `AiRegistry` select portable local/OpenAI
engines while reserving Anthropic for future text tasks. Cloud payloads over 25 MiB are rejected
before HTTP and rate-limit errors remain portable for fallback handling.

Issue #24 extends the headless server with device observations, 24-hour per-device metric series,
and aggregate power-aware telemetry derived exclusively from the existing `MetricsRepository` port.
The Next.js `/dashboard` client renders responsive Recharts views, status-colored device cards, and
summary values; it treats `telemetry_updated` WebSocket events as a coalesced ten-second refresh
signal. Node verification remains in the existing Node 22 GitHub Actions workflow.

Issue #25 exposes privacy-safe telemetry through caller-freed Flutter FFI JSON. A Material 3
dashboard uses an injectable bridge, ten-second polling, responsive layouts, and `fl_chart`.

