<!-- This directory defines typed domain entities used independently of databases, networks, or AI SDKs. -->

# Domain models

`SyncMessage` is the versioned, length-delimited binary envelope for Automerge sync-protocol bytes.
It includes the target `MeetingId`, allowing an encrypted transport receiver to route a message
without exposing Automerge or libp2p values across the application boundary.

`models.rs` contains pure values for meetings, recordings, transcripts, portable audio-capture data, and non-sensitive compute manifests. `AudioConfig`, `AudioChunk`, and `AudioState` intentionally avoid operating-system stream types so local and P2P capture adapters can be exchanged without changing application consumers. `DeviceManifest` records only a device ID, abstract capacity, and advertised model names; a future encrypted P2P adapter can broadcast it without exposing raw audio, weights, credentials, or hardware serial numbers.

`SpeakerIdentity` is an immutable UUID plus a user-visible display name. `VoiceEmbedding` belongs
to one identity and identifies one microphone/environment-specific signature variant. It contains
only model-derived numeric features, never raw audio. `VoiceMatchResult` represents the
application-visible confidence category without revealing vector-search implementation details.

The models here are immutable value objects representing meeting recordings, generated transcripts, voice identities, and device identity metadata.
