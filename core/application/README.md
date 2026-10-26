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

`file_sync_manager.rs` handles attachment references after CRDT persistence, records
meeting-scoped serving authorization, and makes a best-effort content-addressed fetch only when a
peer advertises the SHA-256 hash. It never imports libp2p or waits in the CRDT apply path. Explicit
64 KiB frames, a 100 MiB limit, two concurrent transfers, and final SHA-256 validation protect the
local CAS commit.

`ai_config.rs` keeps per-device defaults and optional task overrides provider-neutral. `ai_registry.rs`
registers only `TranscriptionEngine` trait objects and selects an explicit provider or its fallback
chain; local is preferred when configured. OpenAI remains adapter-only, while Anthropic is reserved
for future text/LLM work and currently returns `ProviderUnsupported` for transcription.

`ComputeDispatcher` retains the last successful transcript per meeting in an in-memory rolling
buffer. Its next transcription request receives that text through `TranscriptionContext`, while a
first chunk or a different meeting receives empty context. The buffer is not persisted, logged, or
shared over the network.

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

## Telemetry collection

`ComputeDispatcher` optionally owns a `MetricsRepository` port. It measures only the selected
`TranscriptionEngine` call; engines return their normal result and never receive telemetry types.
After a successful result, the dispatcher saves a `TaskMetrics` record with device, task/model
labels, duration, end-to-end latency, and completion timestamp. It uses a documented coarse power
heuristic: CPU-only manifests use 5 W and manifests advertising GPU VRAM use 25 W, converted to
mWh from measured milliseconds. The heuristic is an aggregate planning signal, not a battery
measurement. SQLite aggregation returns total processing time and integer-truncated average
latency without retaining audio, transcript content, or embeddings.

## Attachment meeting suggestions

`AttachmentManager` receives a local file path but delegates timestamp acquisition exclusively to
`MetadataExtractor`; it never reads EXIF, PDF, or filesystem metadata directly. It passes that UTC
timestamp to `NoteRepository::get_meetings_overlapping`, then ranks each inclusive candidate window
by its midpoint. Confidence is `1 - distance_from_midpoint / half_window_duration`: an exact midpoint
scores `1.0`, either edge scores `0.0`, and intermediate times decay linearly. Results are sorted by
descending score with meeting ID as a deterministic tie-breaker. The manager logs only processing and
suggestion counts, never paths or file contents, and returned candidates remain subject to user
confirmation.

## External meeting bridge and guaranteed identity

`TeamsProcessor` is provider-neutral application orchestration for external meeting bridges. It
subscribes only through `MeetingBridge`, passes each isolated `ExternalMediaChunk` to an
`IdentityMapper`, derives a voice vector through `VoiceEmbeddingExtractor`, stores a fresh variant
through `VoiceVectorDB`, and forwards the chunk to `TranscriptionEngine`. It does not import or
name a Microsoft, Graph, Azure, or bot SDK.

`TeamsIdentityMapper` treats `ExternalParticipant.external_id` as the durable mapping key. An
unseen key creates a new UUID `SpeakerIdentity` whose initial name is the participant's
`display_name`; an existing key returns its existing local identity unchanged. Because an external
bridge guarantees one participant per audio stream, this flow bypasses the local
diarization/weak-match/manual-confirmation path and enrolls a new embedding variant automatically.
`SlideCapture` values route only through `AttachmentWorkflow`, preserving an adapter boundary for
the attachment manager and keeping image bytes out of logs.
