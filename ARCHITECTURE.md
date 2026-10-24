/core (Rust)
├── domain/           # PURE BUSINESS LOGIC (No external crates)
│   ├── models/       # Meeting, Note, VoiceSignature structs
│   └── errors/       # Domain-specific error enums
├── ports/            # INTERFACES (Traits)
│   ├── ai.rs         # trait TranscriptionEngine, trait DiarizationEngine
│   ├── network.rs    # trait P2PNetwork, trait PeerDiscovery
│   └── storage.rs    # trait NoteRepository, trait VoiceVectorDB
├── application/      # USE CASES / ORCHESTRATION
│   ├── meeting_recorder.rs # Orchestrates audio -> AI -> storage
│   └── compute_router.rs   # Implements the Dispatcher pattern
├── adapters/         # IMPLEMENTATIONS (The "Plumbing")
│   ├── ai/           # whisper_adapter.rs, openai_adapter.rs
│   ├── network/      # libp2p_adapter.rs
│   └── storage/      # sqlite_adapter.rs, automerge_adapter.rs
└── bin/              # ENTRY POINTS
    ├── cli.rs        # Linux CLI
    └── daemon.rs     # Headless server / FFI bridge for Flutter

/ui (Flutter)
├── presentation/     # UI Widgets (Dumb, just renders state)
├── controllers/      # State management (Riverpod/Bloc)
└── infrastructure/   # FFI bindings to call the Rust /core

/server (React/Next.js)
└── src/
    ├── components/   # Web UI
    └── api/          # REST/WebSocket wrappers for the Rust daemon