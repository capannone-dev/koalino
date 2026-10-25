<!-- This directory contains use-case orchestration that coordinates domain types through ports. -->

# Application layer

Application services implement workflows such as recording processing and compute selection. They contain no direct infrastructure integrations. `compute_router.rs` selects an injected `TranscriptionEngine` port from portable local/remote `DeviceManifest` values; concrete model and encrypted-P2P implementations remain outside the application layer.

`network_events.rs` consumes only `NetworkEvent` values supplied by a `P2pNetwork` port and keeps
an application-visible reachability view. It has no dependency on libp2p identities, swarm events,
or socket APIs.

`sync_manager.rs` drives the Automerge sync handshake for one meeting document. On discovery it
generates a peer-specific handshake; on a received envelope it validates the protocol, applies the
message through `MeetingCrdtService`, saves the merged document through `NoteRepository`, and sends
the next acknowledgement when required. After a locally persisted edit, `notify_local_change()`
broadcasts a fresh encrypted `SyncMessage`. The manager uses only `P2pNetwork` and
`NoteRepository`, so its tests use mocks and never open sockets or SQLite files.

`voice_identity_manager.rs` depends only on `VoiceVectorDB`, `VoiceEmbedding`, and
`VoiceMatchResult`. The storage adapter calculates similarity; the manager classifies the best
returned candidate as strong only above `0.80`, weak from `0.50` through `0.80` inclusive, and no
match below `0.50`. A confirmed weak match creates a UUID-distinct embedding variant under the
user-confirmed existing identity, allowing microphone and room changes to be learned without
changing the identity.

`diarization_processor.rs` receives opaque audio bytes and a meeting ID, then coordinates only the
`DiarizationEngine`, `VoiceIdentityMatcher`, and `NoteRepository` ports. It turns each engine
segment into either a strong identity attribution or a temporary `UnassignedVoiceClip`. Weak and
absent matches retain the source bytes plus a millisecond range so manual review can play the
relevant interval and assign an identity later; the processor never logs audio or embeddings.
