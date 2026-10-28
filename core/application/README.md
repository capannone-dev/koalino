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

`meeting_crdt.rs` owns the meeting document schema. The root map holds `note_text` and, since
Issue #39, an optional `title` scalar string. `create_titled_meeting` and `set_meeting_title` write
the CRDT first and then the SQLite `meetings.title` column, which is only a local projection kept
for fast list and detail reads. Every merged peer change (`apply_sync_message`,
`merge_remote_document`) projects the winning title back into SQLite.

- **Conflicts:** `title` is an Automerge register, not `Text`. Concurrent renames resolve to the
  operation with the highest Lamport counter, with ties broken by actor ID, so every peer
  converges on the same whole title. A rename made after seeing another always wins.
  Character-level `Text` merging was rejected because two concurrent renames would interleave into
  a title nobody typed.
- **Legacy documents:** a document without `title` syncs unchanged. Its meeting stays untitled and
  shows the dated `display_title()` fallback, which is never written into the CRDT, because as a
  real value it could beat a peer's real title.
- **SQLite-only titles:** a title stored only in SQLite (Issue #35) is copied into a document with
  no `title` key the first time a sync message is generated.
- **Invalid peer values:** a value that is not a valid `MeetingTitle` is ignored. It is never
  projected and never fails the sync.
- **Cost:** a rename is one `put`; a no-op rename adds no change. Tests measure about 250 bytes for
  the change-carrying message and 720 bytes for a whole fresh handshake.

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

## Meeting export workflow

`export_manager.rs` orchestrates Markdown, PDF, and JSON exports. It talks only to ports:
`NoteRepository` (meeting metadata, transcript segments, flat transcript), `AttachmentRepository`,
`VoiceVectorDB` (display names only, never vectors), and `MeetingExporter`.

1. **Validate the format.** `ExportWriter::ensure_format_supported` rejects formats with no
   configured exporter (including the reserved `Srt`) with `ExportError::UnsupportedFormat`
   before anything is read or written.
2. **Select meetings.** `export_meeting` takes one ID, `export_meetings` a list, and `export_all`
   every meeting whose creation time lies in `ExportOptions::date_range` (all meetings without a
   range). An empty selection fails with `ExportError::NoMeetingsSelected`.
3. **Extract.** `extract_meeting` returns `ExportError::MeetingNotFound` for unknown meetings. It
   prefers stored `TranscriptSegment`s, labelling speakers by resolved identity name, as
   `Unknown speaker` (identity no longer in the voice store), or as `Unattributed` (no identity).
   Without segments, a non-empty flat transcript becomes one unattributed segment; with neither,
   the export has an empty transcript. Attachments are loaded only when
   `include_attachments` is set. Titles equal the meeting ID because no title is stored.
4. **Stage attachments (`ExportWriter::write`).** Markdown copies sources into
   `<export stem>_attachments/` beside the file so relative links keep working when the folder is
   moved. PDF copies into a unique OS temp directory that is always deleted afterwards. JSON copies
   nothing and lists source paths. Unreadable sources are logged by attachment ID and rendered as
   unavailable. Source files are never moved or modified.
5. **Name and write.** Generated names are `{meeting date}_{label}_{export timestamp}.{ext}`
   (or `meetings-export_{count}_{timestamp}.{ext}` for batches), sanitized to `[A-Za-z0-9._-]`,
   with `-N` appended on collision. `label` is the stored meeting title, truncated to 80
   characters (Issue #35). Untitled meetings, and titles without ASCII letters or digits, use the
   meeting ID instead, because the dated fallback title would only repeat the date prefix.
   Documents always carry the display title (stored or fallback), which the renderers use as the
   Markdown heading, PDF title, and JSON `meeting.title`. An explicit destination overrides the generated name.

`ExportManager` only borrows its `ExportWriter`, so a driver can extract under a repository lock,
drop the manager, and render on a blocking thread (the HTTP API does this with `spawn_blocking`). Errors are
`anyhow` values wrapping `ExportError`, so interfaces can `downcast_ref::<ExportError>()` to map
not-found, bad-request, and internal failures.

`pipeline/` connects transcription, diarization, voice identity, and segment persistence.
`MeetingSession` turns each VAD chunk or external bridge chunk into speaker-attributed
`TranscriptSegment` rows. See `pipeline/README.md` for the flow, alignment policy, and failure
handling. `PipelineFactory` (Issue #32) is the pipeline's composition root.

## Live recordings

`recording_manager.rs` (Issue #32) runs pipeline sessions for live recordings on one dedicated
worker thread. The port traits are not `Send`, so the worker builds its `PipelineFactory` from a
`Send` builder closure and keeps it (and any loaded model) for its whole life. Callers send FIFO
commands: `begin_recording` opens a session, `submit_chunk` queues a `VadAudioChunk` without
waiting, and `end_recording` returns a `RecordingSummary` only after every earlier chunk is
processed. A failing chunk is logged and counted while the recording continues. Dropping the
manager drains an active recording before the worker exits. `with_note_repository` runs a query
on the worker's repository. Transcription never runs on a UI thread or the async runtime.

Separately from the local recording, the worker holds at most one *external* meeting session
for the Teams bridge (Issue #33). `begin_external_meeting(meeting, SharedIdentityMapper)` opens
it: it creates the meeting, resumes it when the same meeting is already open (bot reconnects),
or ends a different one. `process_external_chunk(ExternalChunkRequest)` runs
`MeetingSession::process_external_chunk` (transcription and identity mapping, no diarization)
and waits for the segment count, which gives the bridge pump its backpressure.
`end_external_meeting` closes the session and returns its summary.

## Unknown-speaker clustering

`unknown_speaker_clusterer.rs` groups a meeting's weak and unmatched voices so the user labels
each person once instead of once per chunk. It persists only through `NoteRepository` and enrolls
identities only through `VoiceVectorDB`.

```text
unknown/pending-review chunk (embedding, chunk_id, duration)
  │
  ├─ validate: non-empty, finite, non-zero ─────── else InvalidEmbedding (session falls back
  │                                                 to an unclustered review clip)
  ├─ load unresolved, non-empty clusters of this meeting only
  ├─ cosine(embedding, representative) per cluster (dimension mismatch → skipped)
  ├─ best score > threshold?  yes → join; representative = (rep·n + e) / (n + 1)
  │                           no  → new cluster, representative = e
  └─ save_unknown_speaker_cluster_chunk (cluster upsert + member row, one transaction)

assign_identity_to_cluster(cluster, identity)
  1. reject unknown or already-resolved clusters
  2. identity_created = identity had no variants; store every member embedding as a variant
  3. reload segments; set speaker_identity_id where unknown_cluster_id == cluster; save
  4. resolve_unknown_speaker_cluster: mark resolved + delete its review clips (raw audio)
```

**Threshold.** `ClusteringConfig` defaults to `DEFAULT_CLUSTER_SIMILARITY_THRESHOLD = 0.70`; a
chunk joins only when its similarity is strictly greater. The value must be finite and in
`(0, 1]`. Raising it splits one person across more clusters, which means more review work but
fewer wrong merges. Lowering it merges more aggressively, risking two similar-sounding people in
one cluster, so one label would mis-attribute the other person. 0.70 sits below the matcher's
0.80 strong-match cutoff because within-meeting voices share a microphone and room, so
same-speaker similarity is usually high. It is a starting point to tune against real embeddings.
Configure it with `MeetingSession::with_clustering_config`.

**Identity creation.** `assign_identity_to_cluster` takes a `SpeakerIdentity`. It saves the
identity through `VoiceVectorDB::save_speaker_identity` before enrolling the cluster's embeddings,
then returns the reloaded, persisted identity. `identity_created` reports whether the voice store
had no record of that UUID before. `TeamsProcessor` likewise saves an unseen participant's
identity before its first variant, without renaming identities that already exist.

**Partial failures.** The assignment steps are not one transaction across ports. Variants are
stored first. If saving segments or resolving fails, a retry re-stores variants, which only adds
duplicate acoustic variants. A cluster is never marked resolved before its segments are relinked.

## Meeting rename

`meeting_rename.rs` (Issue #38) is the one use case behind `meetings rename` (CLI),
`PATCH /api/meetings/{id}` (HTTP), and `note_taker_rename_meeting` (FFI).

- `rename_meeting` validates the text with `MeetingTitle::parse` before any I/O, so an invalid
  title never touches storage.
- It loads the meeting's metadata (an unknown meeting is `MeetingNotFound`), then writes the title
  through `MeetingCrdtService::set_meeting_title`: the replicated Automerge title first (Issue
  #39), then the SQLite projection. Peers receive the rename on their next sync.
- It returns `MeetingRename` with the previous display title (the dated fallback when the meeting
  was untitled) and the new title, so adapters can confirm the change. Titles are never logged.

## Cluster review

`cluster_review.rs` is the one use case behind the CLI (`voice list-clusters`,
`voice assign-cluster`), HTTP (`GET /api/meetings/{id}/unknown-clusters`,
`POST /api/clusters/{id}/assign`), and FFI (`note_taker_get_unknown_clusters`,
`note_taker_assign_cluster`) commands. Those driving adapters only parse and format.

- `list_unknown_clusters` calls `get_clusters_for_meeting`. It returns `UnknownClusterSummary`
  values (ID, meeting, total duration, chunk count, creation time) and drops the biometric
  representative vector.
- `assign_cluster_to_named_identity` trims the name and rejects a blank one before writing
  anything. A name matching an identity from `VoiceVectorDB::list_speaker_identities` (trimmed,
  ignoring case) reuses that identity. Any other name gets a fresh UUID. It then calls
  `assign_identity_to_cluster` and returns a `NamedClusterAssignment`.

Both functions accept unsized (`dyn`) ports, as the HTTP server holds them, through the
`&T` forwarding impls of `NoteRepository` and `VoiceVectorDB`. With a persistent voice store
(`SqliteVoiceVectorDB`, as composed by the CLI, daemon, and FFI), an assigned name survives
restarts and is reused by later assignments.

## Embedding encryption

`crypto/` (Issue #31) holds `VoiceEncryptor`, which seals embeddings with AES-256-GCM and
implements the `EmbeddingCipher` port, and `KeyManager`, which resolves the key from
`NOTE_TAKER_ENCRYPTION_PASSPHRASE` (Argon2id) or a random local key file and verifies it against
the database's key-check value. Storage adapters receive the cipher through the port and never
handle keys. The scheme and the key-storage tradeoff are documented in `crypto/README.md`.
