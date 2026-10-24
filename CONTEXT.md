<!-- Captures the current implementation boundary for contributors resuming work on the project. -->

# Engineering context

Issues #1 and #2 establish the Rust core and its first storage adapter. `core/domain` is pure business code; `core/ports` provides interfaces; `core/application` coordinates ports; `core/adapters` contains infrastructure. `core/adapters/storage/sqlite_adapter.rs` implements the `NoteRepository` port with bundled SQLite and stores serialized Automerge documents as opaque BLOBs. `core/application/meeting_crdt.rs` creates, edits, loads, and merges those documents without exposing Automerge across a port boundary. AI, libp2p, Flutter, and server adapters remain unimplemented.

Logging emits JSON to stdout and a daily `app_log` rolling file. The `LoggingRuntime` provides a reloadable filter for a future UI or CLI runtime-control surface. Events must include `device_id` and must never log raw audio, voice embeddings, E2EE keys, or other sensitive values.

The SQLite adapter logs initialization and completed persistence operations with device and meeting identifiers only. It stores creation time and acquisition-device metadata, rejects orphan writes, and does not yet store voice embeddings; a future implementation must encrypt voice embeddings at rest with AES-256-GCM.
