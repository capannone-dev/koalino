<!-- This directory contains storage adapters that implement domain-neutral persistence ports. -->

# Storage adapters

`sqlite_adapter.rs` implements `NoteRepository` with SQLite. SQLite types remain entirely inside this adapter; application code communicates through `core/ports/storage.rs`.

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

The adapter rejects writes to unknown meetings rather than silently creating orphan notes. It emits structured `tracing` events with operation, device, and meeting identifiers only—never transcript content or CRDT bytes.

The `attachments` table persists only an attachment UUID, optional confirmed meeting ID, local
path, MIME type, original UTC timestamp, and source-device metadata. It never stores attachment
bytes. `AttachmentRepository` checks referenced meetings before accepting a confirmed link.

`in_memory_voice_vector_db.rs` implements `VoiceVectorDB` with a process-local collection and a
simple cosine-similarity loop. It is suitable for deterministic unit tests and development only;
it does not provide persistence or AES-256-GCM encryption and must never be selected as the
production biometric store.

The same SQLite adapter implements `MetricsRepository`. Its `task_metrics` table stores only task
and device identifiers, model labels, timings, coarse power estimates, and timestamps; it contains
no recordings, transcript text, raw audio, or embeddings. `device_load_stats` stores the latest
non-sensitive CPU/GPU/task-count snapshot for each device. Both tables are exercised through the
adapter's in-memory SQLite constructor in unit tests.
