<!-- This directory declares interfaces implemented by infrastructure adapters and consumed by application use cases. -->

# Ports layer

Ports define the stable contracts for audio capture, AI processing, P2P networking, and persistence. They prevent domain and application code from depending on OS audio APIs, libp2p, databases, or AI SDKs.

`TranscriptionEngine::transcribe` accepts a portable `TranscriptionContext` beside recording metadata. Application callers provide an empty context for standalone work; `ComputeDispatcher` supplies the prior successful same-meeting chunk transcript. AI adapters map that text to their provider-specific prompt mechanisms without exposing provider types to the application.

The P2P port exposes `NetworkEvent` values for discovery, expiry, and future opaque payload
delivery. Its nonblocking `poll_event` operation lets application tests inject a mock network
without starting libp2p or opening sockets.

`broadcast_encrypted` provides application-level fan-out of opaque protocol envelopes only through
authenticated encrypted peers. It never gives application services access to libp2p topics,
connections, or socket handles.

`VoiceVectorDB` accepts and returns `VoiceEmbedding` values only. Its adapter owns all vector
similarity calculations and biometric persistence details, while the application receives just
candidate scores through the port.

`DiarizationEngine` is an async AI port that accepts opaque audio bytes and returns portable
`SpeakerSegment` timing plus derived embeddings. `VoiceIdentityMatcher` exposes only conservative
identity classifications, allowing `DiarizationProcessor` tests to replace every external concern
with deterministic mocks. `NoteRepository` persists temporary `UnassignedVoiceClip` values for
weak or absent classifications without exposing storage implementation types.

`MetricsRepository` persists only privacy-safe completed-task metadata and the latest device-load
snapshot. Application services use it to record duration, latency, coarse energy estimates, and
aggregation data without passing AI inputs, outputs, raw audio, transcript content, or embeddings.

`MetadataExtractor` reads a UTC capture timestamp through an adapter-defined file-metadata
implementation. `NoteRepository::get_meetings_overlapping` returns only portable `MeetingWindow`
values, and `AttachmentRepository` stores attachment metadata and links without ever reading file
contents. This keeps EXIF, PDF, filesystem, and database details out of attachment orchestration.

`external_bridge.rs` defines `MeetingBridge` for standard-library receivers of portable external
media and slide values, never a provider SDK. `IdentityMapper` maps the stable
`ExternalParticipant.external_id` to a local `SpeakerIdentity`, auto-creating it from
`display_name` through `ExternalIdentityRepository` when needed. `VoiceEmbeddingExtractor` and
`AttachmentWorkflow` keep isolated-audio enrollment and slide routing behind independently mocked
ports.
