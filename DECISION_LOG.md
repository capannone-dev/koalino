<!-- Records architectural decisions that contributors should read before modifying the implementation. -->

# Decision log

## 2026-10-25 — Issue #1 Rust core scaffold

- Adopted the documented hexagonal layout: domain, ports, application, adapters, and binary composition roots.
- Kept AI, P2P, storage, and CRDT integrations behind traits; no heavyweight model dependencies were added.
- Selected `tracing`, `tracing-subscriber`, and `tracing-appender` for required JSON stdout and daily rolling-file logs. The filter is reloadable at runtime.
- The repository currently spells the requirements file `REQUIRMENTS.md`; links retain that existing filename to avoid an unrelated rename.

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
