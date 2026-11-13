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
| #22 OpenAI Whisper configuration and cloud adapter | Removed (#117) | Transcription is on the device only; the cloud adapter and its key were removed. |
| #27 Speaker-attributed transcript pipeline | Ready for review | `MeetingSession` wires transcription, optional diarization, voice matching, VAD-anchored alignment, and segment persistence; Teams chunks use mapped identities without diarization; mock-port TDD plus an in-memory SQLite three-chunk integration test. |
| #32 Wire MeetingSession into the recording flow | Ready for review | `PipelineFactory` composition root (meeting creation, optional diarization/matcher, shared clustering policy); `RecordingManager` worker with FIFO commands and draining stop; `RecordingController` capture driver with delete-on-read `ChunkAudioStore`; env-driven `pipeline_composition`; FFI pipeline start, `push_audio_pcm`, and status `4`; CLI foreground `record start --input <wav|->` with cross-process `record stop`; daemon start-up composition and graceful shutdown. Follow-ups: microphone and diarization adapters. |
| #33 Teams bot ↔ Rust protocol (WebSocket + Protobuf) | Ready for review | `teams_bridge.proto` v1 with golden `protoc` fixtures and checked-in rust-protobuf code (freshness test); localhost `tokio-tungstenite` server (Origin refusal, optional shared-secret JSON auth with 1008, single active bot, reconnect-safe UUIDv5 meetings); validated decoder; non-blocking drop-oldest `BridgeBuffer`; `RecordingManager` external meeting session; SQLite `ExternalIdentityRepository`; `SlideAttachmentStore`; daemon composition from `NOTE_TAKER_TEAMS_BRIDGE_*`; mock-bot integration test (5 chunks → 5 Known segments + slide). Follow-up: the bot itself (#34). |
| #34 C# Teams bot: audio/video extraction and streaming | Ready for review | `teams-bot/` .NET 8 solution: `TeamsBot.Core` (16 kHz mono per-speaker `AudioProcessor`, NV12→JPEG `VideoProcessor` at 1 fps/q80 with change skipping, `ProtobufSerializer` byte-identical to the Rust fixtures, `WebSocketClient` with shared-secret auth, 1→30 s jittered reconnect, metadata resend, 30 s drop-oldest buffer, `BotService` lifecycle with recording notice, roster tracking, and resilient ends, MSAL token cache + 401 retry, join-link parser); `TeamsBot.Media` Graph Communications + Real-Time Media adapter (Windows); ASP.NET Core host (Serilog, `/api/calling`, API-key `/api/join`); xUnit + NSubstitute suite with a Kestrel mock bridge and an end-to-end integration test; Linux + Windows CI; README with Azure AD/Bot/manifest setup, deployment, and manual meeting test. Follow-up: run the manual real-meeting test on a Windows host. |
| #23 VAD-based chunking and context-aware transcription | Ready for review | Feature-gated WebRTC VAD state machine, language-sensitive pause cuts, soft-max lookback, graceful time fallback, capability advertisement, and context propagation are implemented. |

## WP5: Flutter UI & FFI

| Issue | Status | Scope |
| --- | --- | --- |
| #5 Initialize Flutter project and Rust FFI bridge | Ready for review | Material 3 recording start UI, bridge-injected widget tests, Dart FFI adapter, and C-compatible Rust recording/log-level commands. |
| #11 Voice Gallery, unassigned playback, and weak match prompting | Ready for review | JSON metadata/path FFI, local native playback, Riverpod controller, Material 3 review screen, and fake-bridge widget tests. |
| #14 Attachment import UI, file picking, and meeting suggestion workflow | Ready for review | File-picker seam, JSON-only AttachmentManager FFI, Riverpod controller parsing, Material 3 ranked confirmation dialog, and fake-boundary widget tests. |
| #25 Flutter telemetry dashboard | Ready for review | Caller-freed telemetry FFI JSON, polling controller, responsive Material 3 fl_chart dashboard, and drawer navigation. |
| #36 Flutter meeting titles | Ready for review | Optional title field on the recording screen (blank → null) via `note_taker_start_recording_with_title`; `RustMeetingBridge` + JSON decoders for `note_taker_get_meetings`/`note_taker_get_meeting`, which now also return `duration_ms` and speaker `segments`; title-first meeting list (phone route, desktop list-detail ≥ 840 px) and bold headline detail header; adaptive `NavigationBar`/`NavigationRail` shell replacing the broken drawer. Verified with `make flutter-lint`/`flutter-test` (42 tests) and a Dart script against the real `cdylib`. Not verified: a GUI run on a device (no emulator, no `cmake`/`ninja`, no transcription engine). |
| Delete meetings and voices | Ready for review | Delete buttons with a confirmation on meeting rows, the open meeting, unknown voices and saved people; records in `deleted_items` and `NTD1` messages stop paired devices restoring them; the Voice Gallery marks the voice playing |
| #41 Flutter meeting rename | Ready for review | `RustMeetingBridge.renameMeeting` binds `note_taker_rename_meeting` on a background isolate and maps statuses 1/2/3/5/other to `MeetingRenameException`; `validateMeetingTitle` mirrors `MeetingTitle::parse` (trim, non-empty, ≤ 200 Unicode characters, no control characters); shared Material 3 `RenameMeetingDialog` (pre-filled and selected, Enter saves, Escape cancels, per-status messages, snack bar); list context menu by long-press, right-click, Shift+F10/context-menu key, and a screen-reader action; pencil beside the detail title; in-place list and detail updates. Verified with `make flutter-lint`/`flutter-test` (77 tests) and a Dart script against the real `cdylib` (statuses 1 and 5). Not verified: a successful rename through the real library (no transcription engine to create a meeting) and a GUI run on a device. |

## WP11: Unknown-speaker clustering

| Issue | Status | Scope |
| --- | --- | --- |
| #29 Expose unknown-speaker cluster listing and identity assignment | Ready for review | Shared `cluster_review` use case; `voice list-clusters`/`assign-cluster` CLI; `GET /api/meetings/{id}/unknown-clusters` and `POST /api/clusters/{id}/assign`; `note_taker_get_unknown_clusters`/`note_taker_assign_cluster` FFI; cluster-card Voice Galleries in Flutter and web. Rust and web verified locally; Flutter verified by `flutter-ci.yml`. |
| #30 Persist speaker identity names in the voice store | Ready for review | `VoiceVectorDB::save_speaker_identity`/`load_speaker_identity`; `SqliteVoiceVectorDB` (`speaker_identities`, `voice_variants`, idempotent migration tested on a pre-#30 database); cluster assignment, Teams auto-enrollment, and FFI `assign_new_identity` persist names; CLI, daemon, and FFI compose the SQLite store, with restart tests for each. |
| #31 Encrypt voice embeddings at rest using AES-256-GCM | Ready for review | `application/crypto` (`VoiceEncryptor`, Argon2id/key-file `KeyManager`); `EmbeddingCipher` and `EncryptionMetadataRepository` ports; all four SQLite embedding columns sealed and bound to their row; `embedding_format` migration of pre-#31 plaintext; key verified before stores open in the CLI, daemon, and FFI; 1000-embedding benchmark under 1 s. |
| #40 Migrate encryption key storage to the platform keychain | Ready for review | `KeychainStore` port and `adapters/keychain::PlatformKeychain` (`keyring` 4: macOS Keychain, Windows Credential Manager target `note-taker:encryption-key`, Linux Secret Service via pure-Rust zbus); `KeyManager` keychain resolution with verified file migration (delete only after an exact read-back), file fallback (warning when unavailable, error on access failure), conflicting-file handling, corrupt-entry rejection, and `KeyMissing`; `NOTE_TAKER_KEYCHAIN_ENABLED`; passphrase mode never touches the keychain. Mock-keychain TDD (16 key-manager and 6 adapter tests); real Secret Service round-trip and migration tests (`--ignored`) and a manual CLI migration run on GNOME Keyring. Not verified: macOS and Windows hosts. |
| #28 Cluster unknown speaker chunks by voice similarity within meetings | Ready for review | `UnknownSpeakerCluster` domain model, cosine-similarity clusterer (0.70 default, running-mean representative), SQLite cluster tables with legacy migration, one review clip per cluster in `MeetingSession`, and cluster identity assignment that enrolls variants, relinks segments, and deletes review audio. |

## WP10: Meeting export

| Issue | Status | Scope |
| --- | --- | --- |
| #35 Meeting titles | Ready for review | Validated `MeetingTitle` and `MeetingMetadata::display_title()` fallback (`Meeting YYYY-MM-DD HH:MM UTC`, computed on read); nullable `meetings.title` with an idempotent migration for older databases; `record start --title` and title-aware `meetings list`/`view`; FFI `note_taker_start_recording_with_title`, `note_taker_get_meetings`, `note_taker_get_meeting`; `title`/`created_at` on `GET /api/meetings[/{id}]` and a new `POST /api/meetings`; title-based export headers and filenames; Teams metadata titles stored on untitled meetings; web meeting pages show titles. Follow-up: replicating titles through the CRDT and a rename command (Dart binding/UI done in #36). |
| #39 Sync meeting titles via P2P CRDT | Ready for review | Optional root `title` register in the Automerge meeting document; `MeetingCrdtService::create_titled_meeting`/`set_meeting_title`/`load_title`; SQLite `meetings.title` projected from every merged peer change; SQLite-only (#35) titles copied into title-less documents before syncing; invalid peer titles ignored; HTTP create, titled recording, and Teams titles routed through the CRDT; 12 two-peer `SyncManager` tests (create, rename, A→B, concurrent and causal conflicts, legacy documents, default title, payload budget). Not verified: a real two-device sync, because `SyncManager` is not yet composed in any entry point. |
| #38 Meeting rename across CLI, FFI, and HTTP | Ready for review | Shared `application::meeting_rename` use case writing through `MeetingCrdtService::set_meeting_title` so renames replicate (#39) (title validated before I/O, unknown meeting → `MeetingNotFound`, previous display title returned); `meetings rename <id> <title>` CLI printing old and new titles; `PATCH /api/meetings/{id}` (`200` summary, `400`/`404` JSON errors, CORS `PATCH`); `note_taker_rename_meeting` FFI with new status `5` (`FFI_MEETING_NOT_FOUND`). Use-case, storage-isolation, CLI, HTTP, and FFI tests; manual CLI run against a scratch database. Follow-ups: Dart binding/UI, web UI. |
| #37 Unicode PDF export | Ready for review | Bundled DejaVu Sans Regular/Bold embedded through printpdf's `font_subsetting` (adds `allsorts`); per-character fallback to `.ttf` fonts in `NOTE_TAKER_PDF_FONT_DIR`, `~/.note_taker/fonts/`, and well-known system paths, loaded only when needed; U+FFFD for uncovered characters and built-in Helvetica if the bundled font is unusable; wrapping by measured glyph widths; rebuilt `ToUnicode` maps (UTF-16 surrogates for emoji, shared CJK glyphs) and a UTF-16 document title; 18 contract tests with OFL subset fixture fonts. Latin exports render faster than the old Helvetica path; a 300-segment mixed CJK/emoji transcript is about 100 KB. Not verified: viewers other than poppler and pypdf, and the macOS/Windows system font paths. |
| #26 Meeting export to Markdown, PDF, and JSON | Ready for review | Persisted speaker-attributed transcript segments, `MeetingExporter` port with Markdown/PDF/JSON adapters, `ExportManager` extraction and attachment staging, and shared CLI/HTTP/FFI entry points with in-memory SQLite tests. |

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
| #42 Web meeting rename | Ready for review | `renameMeeting` client for `PATCH /api/meetings/{id}` with typed `MeetingRenameError` kinds (400 invalid title, 404 not found, 5xx server, other, network); `validateMeetingTitle` mirroring `MeetingTitle::parse` (code-point length); shared accessible rename modal (pre-filled and selected, Enter/Escape, focus trap and return, per-failure messages, bottom sheet on phones); "⋯" actions menu and right-click on `/meetings` rows; pencil beside the `/meetings/<id>` heading; in-place updates. `web-ci.yml` now caches npm downloads and lists each test with the verbose reporter. Verified locally: 53 Vitest tests, lint, `next build`, `make rust-lint`/`rust-test`, the client against a scratch daemon (200/400/404/network, CORS preflight), and SSR pages screenshotted in headless Firefox at phone and desktop widths. Not verified: driving the modal in a real browser (no browser automation available). |

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

## v1.0 remaining work (audit, 2026-10-28)

Prioritized by the audit's rule: Security > Privacy > Reliability > Performance > Convenience. See
`AUDIT_V1.md` for the requirement-by-requirement matrix.

| Priority | Issue | Work | Requirement |
| --- | --- | --- | --- |
| P0 | #43 | Done (2026-10-29): pairing with an allowlist, Automerge sync in the daemon and apps, `/api/p2p/*`, Flutter and web Devices screens, CLI `devices list`/`revoke` (applied by a running daemon within a second), and `DeviceManifest` exchange shown on the Devices screens. Follow-ups: #55 relay, #56 voice-library sync, #57 attachment sync, #58 Nearby during pairing, #59 routing by manifests | 3.3, 3.4, 4.1 |
| P0 | #44 | Done (2026-10-30): real diarization adapter (pyannote segmentation-3.0 and SpeechBrain ECAPA on ONNX Runtime, `diarization` feature); clustering threshold retuned from 0.70 to 0.40 | 3.2 |
| P0 | #45 | Microphone capture: `cpal` in the Rust core (`microphone` feature), Android `RECORD_AUDIO`; done | 3.1 |
| P1 | #46 | Settings on Linux, Windows, Android, and the web: runtime log level and Share/Export Logs; done | 6.1, 6.2 |
| P1 | #47 | Android foreground service and lock-screen/notification Start/Pause/Stop; done | 2.1 |
| P1 | — | Embedding key in the iOS Keychain (Android Keystore done 2026-10-28); re-keying and passphrase change | 4.1 |
| P1 | — | Run the #40 keychain checks on macOS and Windows hosts | 4.1 |
| P2 | #48 | Flutter export binding and UI for `note_taker_export_meeting` (Markdown, PDF, JSON from the meeting's Export menu); done | 3.3.1 |
| P0 | #51 | No repeated words at chunk boundaries: no overlap, quiet-point cuts, silent chunks skipped, no previous-text prompt, VAD in app builds; done | 3.1 |
| P1 | #52 | Live transcript on the Record screen, chunk by chunk (chunks at most 15 s); per-speaker colours wait for #44; done | 3.1 |
| P1 | #53 | Telemetry recorded per transcribed chunk; Telemetry moved into Settings (app and web); done | 6 |
| P2 | #49 | Local screen-share slides outside Teams: harness done (detector/capturer/sink ports, no-op adapters, slide-change heuristic, per-recording watcher); a conferencing-app adapter waits for the owner's choice of app | 3.1 |
| P2 | #50 | Name unknown speakers on any paired device: clusters, embeddings and review clips shared sealed with a per-pair X25519/HKDF key, clip audio encrypted at rest, resolutions synced in the meeting document; done | 3.2, 3.3 |
| P1 | #54 | In-app camera (multi-shot, `camera` plugin; Android, iOS, Windows) and multi-file attachments during and after a meeting, placed in the transcript by meeting time; Linux attaches files only; done | 3.1 |
| P1 | #55 | Done (2026-10-30): relay through the owner's own daemon (circuit relay v2 + hole punching) for devices that cannot connect directly | 4.1 |
| P1 | #56 | Done (2026-10-30): voice-library sync between paired devices (union merge, embeddings re-sealed on receipt) | 3.3 |
| P2 | #57 | Done (2026-10-30): sealed attachment lists, files pulled over `FileTransferNetwork`, verified by SHA-256, fetched again after reconnecting | 3.3 |
| P3 | #58 | Done (2026-10-30): devices with a pending pairing are left out of Nearby | 4.1 |
| P3 | #59 | Done (2026-10-30): transcription on a paired device, chosen in Settings, falling back to local | 3.4 |
| P3 | #60 | Done (2026-10-30): diarization and its models in the Linux, Windows and Android installers and the server build; Windows loads `onnxruntime.dll` at run time | 3.4 |
| P3 | #62 | Done (2026-10-30): choose the transcription language before and during a recording, with a default in Settings and AUTO | 3.4 |
| P2 | #63 | Done (2026-11-01): Voices destination in the apps and web; naming, Play and recognition checked on Linux, Windows and Android | 3.2 |
| P2 | #64 | Done (2026-10-30): relay and device addresses in Settings, applied live, in release builds with no defines | 4.1 |
| P2 | #65 | Done (2026-10-30): pairing invites as a link and QR display; the code comparison is still required | 4.1 |
| P3 | #66 | Done (2026-10-30): Nearby and disconnected devices explain what they need | 4.1 |
| P2 | #75 | Efficient mode built and checked in the app on an S23 (2026-11-01): battery figures and in-app speed still open | 3.5 |
| P3 | #68 | Telemetry-based proposals built (2026-11-01): the real-device runs are still open | 3.5 |
| P3 | #61 | Open: measure on real meetings and retune the voice matcher cutoffs; first 15 recordings measured; needs the owner's natural recordings (#72) | 3.4 |
| P2 | #79 | Real paired sync and desktop buttons checked (2026-11-01): the one-time rewrite on a phone is still unconfirmed | 3.2 |
| P3 | #81 | Busy-machine test failures fixed: speed checks count thread CPU time, relay checks wait for progress; 50 of 50 runs pass beside a compile | — |
| P1 | #82 | One person's unknown voices share a card and are named once (grouped across chunks, speakers and meetings at 0.5); the final pass also removes wordless live clusters | 3.2 |
| P1 | #83 | Leftover voices of finished recordings deleted at start-up; recordings need 1/15 of their speech (8–30 s) per speaker, and a brief speaker heard only in isolated seconds joins the nearest | 3.2 |
| P2 | #84 | Devices is a page of Settings in the apps and on the web; the phone's bar keeps room for Projects (#85) | 2 |
| P1 | #85 | Done (2026-11-03): Projects, folders of meetings with files, notes, PDFs, pictures and e-mails (`.eml`, and shared from Android apps), viewed in the apps, synced between paired devices, and on the web; checked on paired VMs and the S23 | 2 |
| P2 | — | Native GNOME (libadwaita-like) and Windows (Fluent) styling for the desktop app; it is Material 3 on every platform today | 2 |
| P3 | — | Persist the attribution category (pending review / unknown / unattributed) with `speaker_identity_id`; relink segments on single-clip `assign_voice_clip` | 3.2 |
| P3 | — | Decide how the Voice Galleries surface unclustered fallback clips | 3.2 |
| P3 | — | PDF export: color emoji, weight-400 instancing of variable fallback fonts, macOS/Windows font paths | 3.3.1 |

Done during the audit: core CI benchmark flake, Whisper language default, persistent device ID,
the structured JSON log format with redaction, the CLI `--log-level` flag and `log-level` command,
the CLI file log at INFO, JSON escaping in the FFI, the web error boundary, Flutter pause/resume/stop,
and keeping every Flutter FFI call off the UI isolate (see `DECISION_LOG.md`, "v1.0 audit").
