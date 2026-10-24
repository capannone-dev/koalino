<!-- Documents stable boundary contracts before external protocol and FFI implementations exist. -->

# API specifications

## Rust core ports

The initial public contracts are Rust traits, not a network protocol:

- `TranscriptionEngine` receives `AudioRecording` metadata and returns a `Transcript`.
- `NoteRepository` persists a `Transcript` locally before future synchronization.
- `P2pNetwork` permits only encrypted opaque-payload transport.
- `VoiceVectorDatabase` accepts embeddings only; it never accepts raw audio.

Concrete libp2p, SQLite, Automerge, model, REST, and FFI protocols will be specified alongside their adapters. No central routing protocol is defined.
