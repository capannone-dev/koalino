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
│   ├── network/      # libp2p_adapter.rs
│   └── storage/      # sqlite_adapter.rs, automerge_adapter.rs
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
 │      strong → Known · weak → pending review + clip · none/error → Unknown + clip  │
 │ 4 alignment: clamp → merge same speaker → tile VAD chunk → split words by time    │
 │ 5 NoteRepository.save_transcript_segments (whole meeting timeline)                │
 └───────────────────────────────────────────────────────────────────────────────────┘
        │
        ▼
 transcript_segments (SQLite) → ExportManager / CLI / HTTP / FFI
```
