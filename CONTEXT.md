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
