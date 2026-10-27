<!-- This directory contains storage adapters that implement domain-neutral persistence ports. -->

# Storage adapters

`sqlite_adapter.rs` implements `NoteRepository` with SQLite; its unknown-speaker cluster SQL lives in the child module `sqlite_unknown_speaker_clusters.rs`. SQLite types remain entirely inside this adapter; application code communicates through `core/ports/storage.rs`.

## SQLite schema

The `meetings` table stores one offline-first meeting per `meeting_id`:

- `created_at_utc`: SQLite-generated UTC creation timestamp.
- `acquisition_device_id`: device metadata captured from the adapter configuration.
- `transcript`: nullable normalized transcript text, populated only after transcription.
- `automerge_document`: non-null serialized Automerge document BLOB.

The `unassigned_voice_clips` table retains temporary review audio for weak or unknown speaker
matches, keyed by a UUID and its owning meeting. Each record includes the segment's start and end
millisecond offsets so a review client can seek to the relevant region without exposing audio to
application logs.

The `transcript_segments` table stores speaker-attributed transcript intervals: owning
`meeting_id`, `start_time_ms`/`end_time_ms` offsets from the meeting start, an optional
`speaker_identity_id` (a `SpeakerIdentity` UUID, `NULL` when unattributed), and `segment_text`.
`save_transcript_segments` replaces a meeting's rows in one transaction; `load_transcript_segments`
returns them ordered by start time. `load_meeting_metadata` exposes the meeting's `created_at_utc`
instant without reading transcript or CRDT payloads. Segment text follows the same at-rest policy
as the `transcript` column.

The `unknown_speaker_clusters` and `cluster_chunks` tables (in
`sqlite_unknown_speaker_clusters.rs`, a child module of the SQLite adapter) group a meeting's
unknown voices:

| Table | Columns |
| --- | --- |
| `unknown_speaker_clusters` | `cluster_id` (PK), `meeting_id` (FK), `representative_embedding` BLOB (running mean), `total_duration_ms`, `created_at_utc` (RFC 3339), `resolved_identity_id` (`NULL` until assigned) |
| `cluster_chunks` | `cluster_id` (FK), `chunk_id` (PK with `cluster_id`), `derived_embedding` BLOB, `duration_ms` |

`save_unknown_speaker_cluster_chunk` upserts the cluster row and inserts the member row in one
transaction, and rejects unknown meetings or a cluster ID owned by another meeting.
`load_unknown_speaker_clusters` lists only unresolved clusters with at least one member, in
creation order. `resolve_unknown_speaker_cluster` sets `resolved_identity_id` and deletes the
cluster's `unassigned_voice_clips` rows in one transaction. `unassigned_voice_clips.cluster_id`
and `transcript_segments.unknown_cluster_id` link clips and segments to their cluster. Databases
created before Issue #28 gain both columns on open through `ALTER TABLE ... ADD COLUMN`.
Embeddings are little-endian `f32` BLOBs, never logged. Like the existing clip embeddings they
are not yet encrypted at rest (tracked follow-up).

The adapter rejects writes to unknown meetings rather than silently creating orphan notes. It emits structured `tracing` events with operation, device, and meeting identifiers only—never transcript content or CRDT bytes.

The `attachments` table persists only an attachment UUID, optional confirmed meeting ID, local
path, MIME type, original UTC timestamp, and source-device metadata. It never stores attachment
bytes. `AttachmentRepository` checks referenced meetings before accepting a confirmed link.

`in_memory_voice_vector_db.rs` implements `VoiceVectorDB` with a process-local collection and a
simple cosine-similarity loop. It is suitable for deterministic unit tests and development only;
it does not provide persistence or AES-256-GCM encryption and must never be selected as the
production biometric store.

`file_storage_adapter.rs` implements `FileStorage` as a SHA-256 content-addressed store. It writes
each verified object through a unique `.transfer-*.tmp` staging file and atomically renames it to
its hash path, so duplicate content consumes one local object and failed writes are cleaned up.

The same SQLite adapter implements `MetricsRepository`. Its `task_metrics` table stores only task
and device identifiers, model labels, timings, coarse power estimates, and timestamps; it contains
no recordings, transcript text, raw audio, or embeddings. `device_load_stats` stores the latest
non-sensitive CPU/GPU/task-count snapshot for each device. Both tables are exercised through the
adapter's in-memory SQLite constructor in unit tests.
