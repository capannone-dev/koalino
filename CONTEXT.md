<!-- Captures the current implementation boundary for contributors resuming work on the project. -->

# Engineering context

Issues #1 and #2 establish the Rust core and its first storage adapter. `core/domain` is pure business code; `core/ports` provides interfaces; `core/application` coordinates ports; `core/adapters` contains infrastructure. `core/adapters/storage/sqlite_adapter.rs` implements the `NoteRepository` port with bundled SQLite and stores serialized Automerge documents as opaque BLOBs. `core/application/meeting_crdt.rs` creates, edits, loads, and merges those documents without exposing Automerge across a port boundary. AI, libp2p, Flutter, and server adapters remain unimplemented.

Logging emits JSON to stdout and a daily `app_log` rolling file. The `LoggingRuntime` provides a reloadable filter for a future UI or CLI runtime-control surface. Events must include `device_id` and must never log raw audio, voice embeddings, E2EE keys, or other sensitive values.

The SQLite adapter logs initialization and completed persistence operations with device and meeting identifiers only. It stores creation time and acquisition-device metadata, rejects orphan writes, and at first did not store voice embeddings; since Issue #31 every stored embedding is encrypted at rest with AES-256-GCM.

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
tests and development; it is not persistent or encrypted. Issue #30 replaced it in the CLI, daemon,
and FFI with `SqliteVoiceVectorDB`.

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

Issue #26 adds meeting export. `NoteRepository` gains a `transcript_segments` table
(`save_`/`load_transcript_segments`) and `load_meeting_metadata`. `ExportManager` extracts
`ExportDocument` values (segments with speaker names from `VoiceVectorDB`, attachments, creation
time) and passes them to `MeetingExporter` adapters for Markdown, PDF (`printpdf` + bundled `lopdf`),
and JSON schema `1.0`. `ExportWriter` stages attachment copies and picks collision-free filenames.
The CLI (`meetings export`/`export-all`), `POST /api/meetings/{id}/export`, and
`note_taker_export_meeting` all share this manager. Meetings without stored segments still fall
back to the flat transcript as one unattributed segment.

Issue #27 connects the pipeline in `core/application/pipeline`. `MeetingSession` holds the meeting
ID and the injected transcription, optional diarization, voice-matcher, and repository ports. It
processes each `VadAudioChunk` in five steps: transcribe, diarize, match voices, align, and persist
through `save_transcript_segments`. The persisted timeline is seeded from storage at start. Strong
matches persist their identity. Weak matches, no match, matcher failures, and diarization failures
persist `None`, and `AttributedSegment` distinguishes them as "Unknown (pending review)",
"Unknown", or "Unattributed". Weak and no-match segments also save review clips. A transcription
failure persists nothing. External bridge chunks skip diarization and use the `IdentityMapper`
identity. Alignment uses VAD chunk bounds as the reference: it clamps regions, merges same-speaker
neighbours, tiles gaps at midpoints, and splits words by duration because the transcription port
has no word timestamps. Nothing wires sessions into the daemon, CLI, or `TeamsProcessor` yet.

Issue #28 clusters unknown speakers per meeting. `application/unknown_speaker_clusterer.rs`
compares each weak or unmatched voice embedding with the meeting's unresolved clusters by cosine
similarity. A score strictly above the configurable threshold (default 0.70) joins the best
cluster and updates its running-mean representative; anything else founds a new cluster.
`MeetingSession` saves one review clip per cluster, carrying `cluster_id`, and persists the link
on every segment as `TranscriptSegment::unknown_cluster_id`. A clustering failure falls back to the
old per-chunk clip. `assign_identity_to_cluster` enrolls every member embedding as a
`VoiceVectorDB` variant (which is how a new identity comes into existence), re-attributes the
cluster's segments, marks the cluster resolved, and deletes its review audio. The SQLite adapter
stores `unknown_speaker_clusters` and `cluster_chunks` and migrates older databases with the two
new link columns. Nothing yet exposes cluster assignment through the CLI, HTTP, or FFI.

Issue #29 exposes cluster review to users. `application/cluster_review.rs` is the single use case
for the CLI (`voice list-clusters`, `voice assign-cluster`), HTTP
(`GET /api/meetings/{id}/unknown-clusters`, `POST /api/clusters/{id}/assign`), and FFI
(`note_taker_get_unknown_clusters`, `note_taker_assign_cluster`). It calls the clusterer's
`get_clusters_for_meeting` and `assign_identity_to_cluster` unchanged. Listing returns
embedding-free summaries. Assignment rejects blank names, reuses an identity whose name matches
(trimmed, ignoring case), and otherwise creates a new identity UUID. The FFI listing materializes each
cluster's review clip for playback and deletes those files after assignment. The Flutter and web
Voice Galleries now show cluster cards with a name field and an Assign button instead of individual
clips. `NoteRepository` gained a `&T` forwarding impl so services accept the server's `dyn` port.

Issue #30 persists speaker identities. `VoiceVectorDB` gained `save_speaker_identity` and
`load_speaker_identity`. `SqliteVoiceVectorDB` stores names in `speaker_identities` and variants in
`voice_variants` in the same database file, and rejects variants for unsaved identities. The
clusterer saves the identity on assignment and returns the persisted record, so names assigned
through the CLI, HTTP, or FFI survive restarts and are reused case-insensitively. `TeamsProcessor`
and FFI `assign_new_identity` also save names. The CLI, daemon, and FFI compose the SQLite store.

Issue #31 encrypts every stored voice embedding (variants, cluster representatives and members,
review-clip embeddings) with AES-256-GCM. `application/crypto` holds `VoiceEncryptor` and
`KeyManager`. The key comes from Argon2id over `NOTE_TAKER_ENCRYPTION_PASSPHRASE`, or else from a
random key file in `~/.note_taker` (`NOTE_TAKER_ENCRYPTION_KEY_DIR` overrides it). The SQLite
adapters call only the `EmbeddingCipher` port. `storage_composition::open_encrypted_stores`
verifies the key against the database's key-check value before opening the stores, and plaintext
rows from older databases are re-encrypted on first open.

Issue #40 moves the random key into the platform keychain. `KeyManager` takes an optional
`KeychainStore` port, implemented by `adapters/keychain::PlatformKeychain` (`keyring` crate:
macOS Keychain, Windows Credential Manager, Linux Secret Service over zbus; service `note-taker`,
account `encryption-key`). `open_encrypted_stores` injects it unless a passphrase is set or
`NOTE_TAKER_KEYCHAIN_ENABLED` is false. An existing key file is stored, read back, and deleted
only on an exact match. An unavailable or failing keychain falls back to the key file with a
logged warning or error. An encrypted database whose key is in neither place fails with
`KeyMissing`. Tests use a mock keychain; composition test configs disable the real one.

Issue #32 wires `MeetingSession` into the recording flow. `PipelineFactory` is the composition
root: it owns the pipeline ports once, creates missing meetings, and degrades without diarization
or a matcher. `RecordingManager` runs sessions on a worker thread with FIFO commands, so stop
drains in-flight chunks. `core/bin/recording_controller.rs` drives capture (recorder → chunker
→ delete-on-read `ChunkAudioStore` → manager), and `pipeline_composition.rs` selects the engine
from the environment. The FFI composes the pipeline on the first start and gains PCM push and
meeting-ID functions. The CLI records a WAV file or stdin in the foreground with a cross-process
`record stop`, and the daemon composes the pipeline at start-up and shuts down gracefully.

Issue #33 adds the Teams bot bridge. A separate bot process streams Protobuf frames
(`core/protocol/teams_bridge.proto`, rust-protobuf 3.7.2, generated code checked in) over a
`tokio-tungstenite` WebSocket bound to 127.0.0.1:

- **Authentication:** an optional shared secret is sent as the first JSON text frame.
- **Decoding:** frames become `ExternalMediaChunk` and `SlideCapture` values.
- **Buffering:** a non-blocking, drop-oldest buffer holds `BUFFER_SECONDS` of audio.
- **Pump:** a thread opens an external meeting session on the `RecordingManager` worker, which
  runs independently of local recordings. It transcribes each chunk with a persistent Teams
  identity mapping (`external_identity_mappings` in the voice store), producing `Known`
  segments, and stores screen-share frames as meeting attachments.
- **Daemon:** starts the bridge when `NOTE_TAKER_TEAMS_BRIDGE_ENABLED` is set and a
  transcription engine is available, and stops it before the pipeline at shutdown.

Issue #34 adds the bot itself in `teams-bot/`, a .NET 8 solution (C# is approved for this
component only):

- **`TeamsBot.Media`:** joins meetings through the Microsoft Graph Communications calling SDK
  with application-hosted Real-Time Media. It runs on Windows only, and needs a public FQDN, a
  certificate, and a media port.
- **`TeamsBot.Core`:** platform-neutral and tested on Linux.
  - Converts unmixed per-speaker audio to 16 kHz mono chunks tagged with Teams user IDs.
  - Samples VBSS screen share at 1 fps into JPEG slides.
  - Streams both with the shared `teams_bridge.proto` over a reconnecting, buffering
    WebSocket client.
- **Host:** an ASP.NET Core app that exposes the Graph webhook and an API-key-protected
  `POST /api/join`.
- **Connection:** the bot reaches the loopback-only Rust server on the same host or through an
  SSH tunnel; plaintext `ws://` to a remote host is refused.

Issue #35 adds meeting titles:

- **Domain:** `MeetingTitle` is a validated newtype (trimmed, 1–200 characters, no control
  characters). `MeetingMetadata` carries `Option<MeetingTitle>`, and `display_title()` falls
  back to `Meeting YYYY-MM-DD HH:MM UTC`. The fallback is computed on read, never stored.
- **Storage:** titles live in a nullable `meetings.title` column. Older databases gain it on
  open, and their meetings stay untitled. `NoteRepository::save_meeting_title` writes it. Titles
  are never logged. Since Issue #39 the column is a projection of the replicated CRDT title.
- **Entry points:**
  - CLI: `record start --title` (through `RecordingController::start_titled`), and
    `meetings list`/`view` show titles.
  - FFI: `note_taker_start_recording_with_title`, `note_taker_get_meetings`, and
    `note_taker_get_meeting`.
  - HTTP: `title` on `GET /api/meetings[/{id}]`, and `POST /api/meetings` creates titled
    meetings.
- **Exports:** the display title is the export heading. A stored title also names the file.
- **Teams:** the bridge stores the Teams title on meetings that have none yet.

Issue #36 brings meeting titles to Flutter:

- **Recording:** the recording screen takes an optional title. Blank becomes `null`, never `""`,
  because Rust rejects an empty title.
- **Meetings:** a new Meetings destination lists titles above their date and duration. The
  detail screen shows the title as a bold headline header and the speaker-attributed transcript.
  - The meeting FFI JSON gained `duration_ms` and, for detail, `segments`.
- **Bridges:** `RustMeetingBridge` decodes the JSON off the UI isolate.
- **Navigation:** an adaptive `HomeShell` replaces the drawer, using `NavigationBar` on phones
  and `NavigationRail` from 840 px. The meeting list becomes list-detail at that width.
- **Gap:** Flutter still neither captures audio nor stops recordings.

Issue #39 replicates meeting titles through the Automerge meeting document:

- **Schema:** an optional root `title` scalar string, next to `note_text`.
  `MeetingCrdtService::create_titled_meeting` and `set_meeting_title` write it, then the SQLite
  `meetings.title` projection. A rename to the current title records no change.
- **Sync:** `apply_sync_message` and `merge_remote_document` project the winning title into SQLite.
  `generate_sync_message` first copies a SQLite-only (pre-#39) title into a document that has no
  `title` key.
- **Conflicts:** the title is a register. Concurrent renames converge on Automerge's winner: the
  highest Lamport counter, then actor ID. A causally later rename wins.
- **Migration:** documents without `title` still sync. Their meetings show the dated display
  fallback, which is never written into the CRDT. Invalid or non-string peer titles are ignored.
- **Writers:** `POST /api/meetings`, titled recording starts, and the Teams bridge use the service,
  so their titles replicate.
- **Gap:** nothing composes `SyncManager` in the daemon, FFI, or CLI yet. Replication is verified
  between two in-memory peers exchanging real sync messages, not between devices.

Issue #38 adds meeting renaming:

- **Use case:** `application/meeting_rename.rs` is shared by every entry point. It validates the
  title with `MeetingTitle::parse` before any I/O, checks that the meeting exists, and writes
  through `MeetingCrdtService::set_meeting_title` (#39): the replicated Automerge title, then
  the SQLite projection. It returns the previous display title. No storage method was added.
- **Entry points:**
  - CLI: `meetings rename <meeting-id> <new-title>` prints `Renamed meeting <id>: "<old>" -> "<new>"`.
  - HTTP: `PATCH /api/meetings/{id}` with `{"title":"..."}` returns `200` with the summary,
    `400` for an invalid title, and `404` for an unknown meeting. CORS now allows `PATCH`.
  - FFI: `note_taker_rename_meeting(meeting_id, new_title)` returns `0`, `1` for invalid input
    or null pointers, `2` for storage failures, `3` for an unavailable runtime, and the new
    `5` (`FFI_MEETING_NOT_FOUND`).
- **Gaps:** renames replicate on the next sync, but nothing composes `SyncManager` yet (see
  #39). There is no Dart binding, Flutter UI, or web UI for renaming yet.

Issue #37 makes PDF exports Unicode-capable:

- **Fonts:** `adapters/export/pdf_fonts.rs` loads the bundled DejaVu Sans Regular and Bold
  (`export/fonts/`, compiled in) as primary faces. Characters they lack use the first fallback
  `.ttf` that covers them. Fallbacks come from `NOTE_TAKER_PDF_FONT_DIR`, then
  `~/.note_taker/fonts/`, then well-known system files, and are read only when a document needs
  them.
- **Degradation:** uncovered characters render as U+FFFD. Unusable font files are skipped.
  An unparsable bundled font falls back to built-in Helvetica with `?`.
- **Rendering:** `pdf_exporter.rs` wraps by measured glyph advances and draws each line as one
  text object with one run per font. It writes glyph IDs directly, because printpdf's
  `write_text` clones the whole font per call. printpdf's `font_subsetting` (adds `allsorts`)
  embeds only the glyphs used.
- **Post-processing:** a lopdf pass rebuilds `ToUnicode` maps when printpdf's would be wrong:
  characters above U+FFFF, or glyphs shared by several code points. It also stores a non-ASCII
  title as UTF-16, and embeds attachments as before.
- **Tests:** `export/pdf_unicode_tests.rs` uses OFL subset fonts in `export/test_fonts/`. The
  shared PDF text extractor in `export/tests.rs` decodes through `ToUnicode`, as viewers do.
- **Gaps:**
  - Emoji are monochrome.
  - Variable fallback fonts render at their default instance.
  - The macOS and Windows system font paths have not been tried on those platforms.

Issue #41 adds meeting renaming to Flutter:

- **Entry points:** the meeting list row's context menu (long-press, right-click, Shift+F10 or
  the context-menu key, and a screen-reader custom action) and a pencil beside the detail title.
  Both open `RenameMeetingDialog` (`ui/lib/presentation/rename_meeting_dialog.dart`).
- **Validation:** `ui/lib/controllers/meeting_title_validation.dart` duplicates
  `MeetingTitle::parse` (trim, non-empty, at most 200 Unicode characters, no category Cc
  characters), so invalid titles never reach FFI. Keep the two in sync if the Rust rules change.
- **Binding:** `RustMeetingBridge.renameMeeting` calls `note_taker_rename_meeting` in
  `Isolate.run`. `meetingRenameExceptionFor` maps statuses 1/2/3/5 and unknown codes to
  `MeetingRenameFailure`.
- **State:** `MeetingListController.renameMeeting` updates the list and the selected detail in
  place and reloads the list when the meeting is gone.
- **Gaps:** a successful rename was not run through the real library (creating a meeting needs a
  transcription engine), and there was no GUI run on a device.

Issue #42 adds meeting renaming to the web dashboard:

- **Entry points:** a "⋯" actions menu (also opened by right-click) on each `/meetings` row, and a
  pencil beside the `/meetings/<id>` heading. Both open `web/components/rename-meeting-dialog.tsx`.
  The server pages pass their fetched titles to the client components `MeetingList` and
  `MeetingTitleHeader`, which update in place from the `PATCH` response.
- **Validation:** `web/lib/meeting-title.ts` duplicates `MeetingTitle::parse`, as Flutter's
  `meeting_title_validation.dart` does. Keep all three in sync if the Rust rules change.
- **Client:** `renameMeeting` in `web/lib/api.ts` maps statuses to `MeetingRenameError` kinds. The
  UI never shows the server's error text.
- **Gaps:** the modal was exercised in jsdom only, not driven in a real browser.

The v1.0 audit (2026-10-28, `AUDIT_V1.md`) leaves the system in this state:

- **Works end to end:** recording through the CLI (WAV or stdin PCM), the daemon, and the FFI;
  VAD chunking; whisper.cpp or OpenAI transcription with language auto-detection; meetings,
  titles, and rename across CLI, HTTP, FFI, Flutter, and web; the voice gallery and unknown-cluster
  review; export (Markdown, Unicode PDF, JSON); telemetry; the Teams bridge; encrypted
  embeddings; and structured, redacted JSON logs whose level can be changed at runtime.
- **Implemented but not composed:** the libp2p swarm, `SyncManager`, `FileSyncManager`, and
  manifest broadcast are exercised only by tests. Composing them needs peer pairing first (#43).
- **Missing:** a real diarization engine (#44), Flutter microphone capture (#45), Flutter Settings and
  log export (#46), Android background/lock-screen controls (#47), Flutter export (#48), and local
  screen-share capture (#49).
- **Conventions added:** every Flutter bridge calls FFI only through `Isolate.run`. Per-database
  control files beside the DB (`.device-id`, `.log-level`, `.recording-stop`) coordinate CLI and
  daemon processes without network routes.
