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
 │   3b UnknownSpeakerClusterer (weak/none/error): cosine > 0.40 joins the meeting's │
 │      best cluster (running-mean representative), else new cluster + review clip   │
 │ 4 alignment: clamp → merge same speaker → tile VAD chunk → assign words by time   │
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
                                                                             else random key:
                                                                               KeychainStore (#40) ◀── adapters::keychain::PlatformKeychain
                                                                                 macOS Keychain / Windows Credential Manager / Secret Service
                                                                               migrate ~/.note_taker/encryption.key → keychain, then delete
                                                                               keychain unavailable → key file fallback
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
     → VadChunker (speech pauses; ≤ 15 s, cut at a quiet frame)  (whisper.cpp | OpenAI)
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
                                     > 0.40 join (running mean) / new        cluster_chunks
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

## Final speaker pass

Live chunks give captions and provisional speakers. When a recording stops, `MeetingSession::finish_meeting` regroups the
recording's voice windows over the whole meeting, reassigns every word, and names each speaker once. Embeddings stay in
memory until then, and no extra audio is kept. A crash leaves the live result.

## One meeting recorded on several devices (#88)

The owner records a meeting on the laptop and, at the same time, on a paired phone placed near the
other person. No device is told that the two belong together. Each records an ordinary meeting
and keeps its speech sealed for the after-meeting pass (#87), and the recordings sync as usual.

```text
laptop records M1 ──┐                         ┌── phone records M2
 (speech sealed)    │   CRDT sync of M1, M2   │   (speech sealed)
                    ▼                         ▼
     after-meeting worker on each device: find_combinations
       M1 and M2 overlap in time and share 3-word runs within 15 s (≥25%;
       ≥10% over nearly the same span) → fold M2 into M1
       (the earlier recording), combine_on = laptop; the fold syncs in M2's document
                    │                         │
   laptop: plan = Combine (waits ≤3 wk        phone: plan = Send{laptop}
   for M2's speech)                           NTR1 parts sealed with the pair key ──►
                    ◄── NTR2 receipt per part  deletes its copy after the last receipt
   RetainedSpeechInbox stores M2's speech (only for a meeting it combines, only from M2's device)
                    │
   combined pass: each recording in 30 s windows, offset refined from shared words,
   each speaker's words from the device that heard them best → M1's transcript; audio deleted
```

- **Recognition is from the transcripts**, not from the audio: the live transcripts are already
  on every device, so no audio leaves a device until the pair is known, and a third paired device
  never decides.
- **One meeting is shown.** Lists hide M2 and show M1 as "Recorded on Laptop + Pixel". Split
  (CLI, HTTP, FFI, Flutter, web) lists M2 again for good; M2's own live transcript is kept, so
  nothing is lost. Deleting M1 deletes M2.
- **Devices are named by the ID written on their meetings.** P2P addresses peers by libp2p ID, so
  each device's manifest now carries both, and the P2P task translates.
- **Late and many devices:** each recording takes its pass alone after 10 minutes, so its
  transcript improves at once, and keeps its sealed speech. A recording that syncs up to three
  weeks later still joins and is combined. Any number of devices join one group; the first
  device that is not a phone combines (every recording notes whether a phone made it).
- **Status:** every app shows, under "Recorded on", how far combining got as that device sees it:
  one device (and until when another can join), waiting for named devices' audio (connected or
  not), ready, still sending, combined, or not combined.
- **Nothing lingers:** speech not sent or not combined within three weeks is deleted, as is speech of a
  meeting split or deleted meanwhile. A phone without a pass model keeps its speech only for that.
- **Limits:** recordings must overlap in content, so a phone that hears only the other person
  still matches the laptop, which hears both. The gain depends on placement: on NOTSOFAR's
  far-field devices, choosing the device per speaker scored the same as one device, while the
  union of all devices bounds what better placement can give (20.3% against 36.9% word error).
