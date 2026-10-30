<!-- This directory contains pure business concepts with no dependencies on infrastructure or external services. -->

# Domain layer

Domain types encode meetings, recordings, transcripts, device identities, voice identities, and domain-specific failures. It must never import ports, application services, or adapters.

Voice-signature values retain only UUID metadata and model-derived floating-point embeddings. They
never retain raw audio; persistence and similarity calculation remain outside this layer.
Review clips (`UnassignedVoiceClip`) are the exception: temporary audio kept only until the
speaker is named. `ClusterShare` groups an unresolved cluster with its member chunks and clips
for copying to a paired device (Issue #50).

Attachment values retain only a user-selected local path and declared metadata. Metadata extraction,
file access, and attachment persistence remain behind ports; meeting suggestions are immutable
domain values based on portable UTC meeting windows.

`peer_trust.rs` (Issue #43) defines the following:

- `DeviceName`: 1–64 characters, trimmed, with no control characters.
- `TrustedPeer`: a peer ID, a name, and the pairing time.
- `PairKey` (Issue #50): the 256-bit key two paired devices agreed. It is wiped on drop, prints
  as redacted, and compares in constant time (`matches`).
- `PairingCode`: the six digits both devices show during numeric-comparison pairing. It is computed
  as SHA-256 over `note-taker/pairing-code/v1\0`, the smaller peer ID, a NUL, and the larger peer
  ID. The first 8 bytes are read as a big-endian integer, taken modulo 1,000,000, and zero-padded.
  A golden test pins this derivation, so every platform must reproduce it exactly.

`transcription_language.rs` defines the language a recording is transcribed in (Issue #62): AUTO or a
code whisper.cpp knows.
