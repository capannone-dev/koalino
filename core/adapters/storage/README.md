<!-- This directory contains storage adapters that implement domain-neutral persistence ports. -->

# Storage adapters

`sqlite_adapter.rs` implements `NoteRepository` with SQLite. SQLite types remain entirely inside this adapter; application code communicates through `core/ports/storage.rs`.

## SQLite schema

The `meetings` table stores one offline-first meeting per `meeting_id`:

- `created_at_utc`: SQLite-generated UTC creation timestamp.
- `acquisition_device_id`: device metadata captured from the adapter configuration.
- `transcript`: nullable normalized transcript text, populated only after transcription.
- `automerge_document`: non-null serialized Automerge document BLOB.

The adapter rejects writes to unknown meetings rather than silently creating orphan notes. It emits structured `tracing` events with operation, device, and meeting identifiers only—never transcript content or CRDT bytes.
