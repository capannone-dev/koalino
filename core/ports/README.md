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
candidate scores through the port. It also persists `SpeakerIdentity` names
(`save_speaker_identity` inserts or renames by UUID, `load_speaker_identity`,
`list_speaker_identities`); `delete_identity` removes a name together with its variants. A
persistent adapter may reject variants for identities that were never saved.

`MeetingDocumentCodec` (`meeting_document.rs`) reads and edits serialized meeting documents for
storage adapters, so they keep the SQLite index in step with the document inside their own
transactions without linking the CRDT library.

`NodeIdentityStore` (`peer_trust.rs`) keeps this node's encoded P2P identity. Implementations seal it
at rest and keep the first stored value, so concurrent first starts agree (Issue #43).
`TrustedPeerStore` in the same file holds the allowlist of paired devices: `trust_peer` (an upsert),
`revoke_peer`, `trusted_peers`, and `is_trusted`.
`PairingTransport` and `PairingEvent` (in the same file) are the network side of pairing:
admission (a pairing window, nearby unpaired devices, trust and revoke) and the
request/answer exchange.

`EmbeddingCipher` (`crypto.rs`) seals and opens embeddings for a named row context. It also seals
arbitrary secrets (`seal_secret`/`open_secret`), such as the P2P node key. Storage
adapters call it for every embedding BLOB and never see keys or cipher types. The implementation
is `application::crypto::VoiceEncryptor`. `EncryptionMetadataRepository` stores the single
non-secret record (key source, Argon2id salt and parameters, key-check value) that
`KeyManager` needs to rebuild and verify the key.

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

`export.rs` defines `MeetingExporter`, the renderer contract for meeting exports. Its
`export_meeting`/`export_meetings` methods receive fully extracted `ExportDocument` values plus
`ExportOptions` and a destination path, and return an `ExportResult` or a portable `ExportError`.
Exporters never receive meeting IDs or repositories, so data selection stays in
`application::export_manager` and format adapters remain pure presentation. `NoteRepository` also
exposes `load_meeting_metadata` and `save_`/`load_transcript_segments` so exports can show
speaker-attributed, timestamped transcripts.

For Issue #28 `NoteRepository` gains default-implemented unknown-speaker cluster methods:
`save_unknown_speaker_cluster_chunk`, `load_unknown_speaker_clusters`,
`load_unknown_speaker_cluster`, `load_unknown_speaker_chunks`, and
`resolve_unknown_speaker_cluster`. No new port was added. The clustering policy, including the
cosine math, lives in `application::unknown_speaker_clusterer`, and the ports only persist its
results. Writes default to `Unavailable`, so repositories without cluster support degrade to
unclustered review clips.

`NoteRepository` (like `VoiceVectorDB`) is implemented for `&T` where `T: NoteRepository + ?Sized`,
forwarding every method. Application services can therefore borrow an adapter or a `dyn` port
object, such as the HTTP server's `dyn ServerRepository`, without owning it.
