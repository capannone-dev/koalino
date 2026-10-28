```bash
/core (Rust)
├── domain/           # PURE BUSINESS LOGIC (No external crates)
│   ├── models/       # Meeting, Note, VoiceSignature structs
│   └── errors/       # Domain-specific error enums
├── ports/            # INTERFACES (Traits)
│   ├── ai.rs         # trait TranscriptionEngine, trait DiarizationEngine
│   ├── network.rs    # trait P2PNetwork, trait PeerDiscovery
│   ├── external_bridge.rs # provider-neutral meeting media and identity contracts
│   └── storage.rs    # trait NoteRepository, trait VoiceVectorDB
├── application/      # USE CASES / ORCHESTRATION
│   ├── meeting_recorder.rs # Orchestrates audio -> AI -> storage
│   ├── pipeline/           # MeetingSession: chunk -> speaker-attributed transcript segments
│   ├── compute_router.rs   # Implements the Dispatcher pattern
│   └── teams_processor.rs  # Maps guaranteed external identities and auto-enrolls variants
├── adapters/         # IMPLEMENTATIONS (The "Plumbing")
│   ├── ai/           # whisper_adapter.rs, openai_adapter.rs
│   ├── network/      # libp2p_adapter.rs, teams_bridge/ (localhost WebSocket + Protobuf)
│   └── storage/      # sqlite_adapter.rs, automerge_adapter.rs
├── protocol/         # teams_bridge.proto (source of truth) + checked-in generated Rust code
└── bin/              # ENTRY POINTS
    ├── cli.rs        # Linux CLI
    └── daemon.rs     # Headless server / FFI bridge for Flutter
└── server/            # Axum REST/WebSocket adapter, dependent only on ports
└── cli/               # Clap driving adapter, dependent only on application services and ports

/ui (Flutter)
├── presentation/     # UI Widgets (Dumb, just renders state)
├── controllers/      # State management (Riverpod/Bloc)
└── infrastructure/   # FFI bindings to call the Rust /core

/server (React/Next.js)
└── src/
    ├── components/   # Web UI
    └── api/          # REST/WebSocket wrappers for the Rust daemon
```
## Connected meeting pipeline (Issue #27)

```text
 Local audio                                   Teams / external bridge
 AudioCapture → VadChunker (adapter)           MeetingBridge → ExternalMediaChunk
        │ VadAudioChunk                                  │
        ▼                                                ▼
 ┌────────────────────── application/pipeline::MeetingSession ──────────────────────┐
 │ 1 TranscriptionEngine.transcribe            1 TranscriptionEngine                 │
 │      └ error → no segment                     .transcribe_external_media          │
 │ 2 DiarizationEngine.diarize                 2 IdentityMapper.map_external_to_local│
 │      └ missing/error → "Unattributed"          (diarization skipped)              │
 │ 3 VoiceIdentityMatcher.match_embedding         └ error → "Unknown"                │
 │      strong → Known · weak → pending review · none/error → Unknown                │
 │   3b UnknownSpeakerClusterer (weak/none/error): cosine > 0.70 joins the meeting's │
 │      best cluster (running-mean representative), else new cluster + review clip   │
 │ 4 alignment: clamp → merge same speaker → tile VAD chunk → split words by time    │
 │ 5 NoteRepository.save_transcript_segments (whole meeting timeline)                │
 └───────────────────────────────────────────────────────────────────────────────────┘
        │
        ▼
 transcript_segments (SQLite) → ExportManager / CLI / HTTP / FFI
```

## Voice embedding encryption at rest (Issue #31)

```text
 CLI / daemon / FFI ─▶ storage_composition::open_encrypted_stores
                        1 KeyManager.unlock(SqliteEncryptionMetadataStore)   passphrase → Argon2id
                                                                             else ~/.note_taker/encryption.key
                          verify key-check value (wrong key → stop)
                        2 SqliteNoteRepository / SqliteVoiceVectorDB (Arc<dyn EmbeddingCipher>)
                          re-encrypt embedding_format = 0 rows, then seal/open every embedding
 ports::crypto::EmbeddingCipher ◀── application::crypto::VoiceEncryptor (AES-256-GCM)
```

## Live recording flow (Issue #32)

```text
 Flutter: note_taker_push_audio_pcm      CLI: record start --input <wav|->      daemon: start-up
             │ 16 kHz mono i16                   │ PcmInput blocks                    │
             ▼                                   ▼                                    ▼
 bin::recording_controller::RecordingController            bin::pipeline_composition (env)
   LocalRecorder (Idle/Recording/Paused/Interrupted/Stopped)     selects TranscriptionEngine
     → VadChunker (speech pauses, or 30 s; 1 s lookback)         (whisper.cpp | OpenAI)
     → ChunkAudioStore (delete-on-read PCM, the engine's audio source)
     → VadAudioChunk (meeting-relative offsets) ─ submit_chunk (non-blocking)
             ▼
 application::recording_manager::RecordingManager — worker thread, FIFO commands
   owns PipelineFactory (engine, optional diarization, voice matcher, repository, clustering)
   begin → create_session (creates the meeting) · chunk → MeetingSession.process_vad_chunk
   end → drains every queued chunk → RecordingSummary; controller wipes unconsumed audio
```

The worker builds the factory itself because the ports are not `Send`; engines and models are
loaded once and reused across recordings. Stop is graceful: it waits for in-flight chunks.

## Unknown-speaker clustering (Issue #28)

```text
 MeetingSession ──unknown voice──▶ application::UnknownSpeakerClusterer ──▶ NoteRepository
                                     cosine vs. meeting clusters             unknown_speaker_clusters
                                     > 0.70 join (running mean) / new        cluster_chunks
 CLI / HTTP / FFI ─▶ application::cluster_review  (list summaries · name → identity, Issue #29)
 CLI / HTTP / FFI ─▶ application::meeting_rename  (MeetingTitle::parse → exists? → MeetingCrdtService::set_meeting_title, Issue #38)
 user assigns identity to cluster ─▶ assign_identity_to_cluster
                                     0 VoiceVectorDB.save_speaker_identity (persist name, #30)
                                     1 VoiceVectorDB.store_embedding × members
                                     2 relink transcript_segments where unknown_cluster_id = cluster
                                     3 resolve cluster + delete its review clips
```

## Teams bot bridge (Issue #33)

```text
 Teams bot process (separate; owns the Teams SDK)
   │ ws://127.0.0.1:8765 (loopback only, no Origin header)
   │ 1 {"auth":"<NOTE_TAKER_TEAMS_BRIDGE_SECRET>"} → {"auth":"ok"} | close 1008
   │ 2 binary frames: TeamsBridgeMessage { MeetingMetadata | AudioChunk | VideoFrame }
   ▼
 adapters::network::teams_bridge
   TeamsWebSocketServer (tokio-tungstenite; one active bot; reconnects keep the meeting)
     → message_decoder (16 kHz s16le, 1–30 s, metadata first; meeting UUID = v5(Teams ID))
     → BridgeBuffer (never blocks the reader; 10 s of audio, drop oldest + warning)
   ▼ teams-bridge-pump thread (bin::teams_bridge_composition)
   MeetingStarted → RecordingManager.begin_external_meeting
   Audio  → ChunkAudioStore("teams-<uuid>") + RecordingManager.process_external_chunk (waits)
              └ MeetingSession.process_external_chunk: transcribe → TeamsIdentityMapper
                (SqliteVoiceVectorDB external_identity_mappings) → Known segment
   Slide  → SlideAttachmentStore → <slides>/<meeting>/slide-….png + attachments row
```

The external meeting session runs on the same pipeline worker as local recordings but
independently of them, so a microphone recording and a Teams meeting can run at the same time.

## Teams bot (Issue #34)

```text
 Teams meeting ── Graph webhook (HTTPS, JWT) + media (TCP 8445, SRTP) ──┐
                                                                        ▼
 teams-bot/ (C#/.NET 8, Windows Server)
   TeamsBot (ASP.NET Core): /api/calling, POST /api/join (X-Api-Key), /healthz, Serilog
   TeamsBot.Media: GraphCallJoiner (Graph Communications SDK, MSAL, built-in JWT validation)
                   GraphCallSession: roster → MSI map, unmixed audio, VBSS NV12, call end
   TeamsBot.Core:  BotService ── updateRecordingStatus → metadata → media → flush on end
                     ├ AudioProcessor: → 16 kHz mono, per-speaker chunks (600 ms gap / 10 s)
                     └ VideoProcessor: 1 fps, skip unchanged, NV12 → JPEG q80 (SkiaSharp)
                   WebSocketClient: auth → metadata (resent on reconnect) → frames;
                     1→30 s jittered backoff; 30 s audio / 16 slide drop-oldest buffer
   │ ws://127.0.0.1:8765 (same host or SSH tunnel), TeamsBridgeMessage frames
   ▼
 Rust TeamsWebSocketServer (Issue #33)
```

The core never references the Teams SDK. It talks to `ICallJoiner`/`ICallSession`, which
`TeamsBot.Media` implements and the tests mock with NSubstitute. The serializer is checked
against the Rust golden fixtures, so both sides share one wire format.
