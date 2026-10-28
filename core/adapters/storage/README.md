<!-- This directory contains storage adapters that implement domain-neutral persistence ports. -->

# Storage adapters

`sqlite_adapter.rs` implements `NoteRepository` with SQLite; its unknown-speaker cluster SQL lives in the child module `sqlite_unknown_speaker_clusters.rs`. SQLite types remain entirely inside this adapter; application code communicates through `core/ports/storage.rs`.

## SQLite schema

The `meetings` table stores one offline-first meeting per `meeting_id`:

- `created_at_utc`: SQLite-generated UTC creation timestamp.
- `acquisition_device_id`: device metadata captured from the adapter configuration.
- `transcript`: nullable normalized transcript text, populated only after transcription.
- `automerge_document`: non-null serialized Automerge document BLOB.
- `title` (Issue #35): nullable user-chosen title, written by `save_meeting_title` (a validated
  `MeetingTitle`: trimmed, 1–200 characters, no control characters) and never logged. `NULL`
  means untitled. `load_meeting_metadata` returns it, and `MeetingMetadata::display_title()`
  then supplies `Meeting YYYY-MM-DD HH:MM UTC` (from `created_at_utc`) for untitled meetings. The
  fallback is computed on read, never stored. Databases created before Issue #35 gain the column
  on open through an idempotent `ALTER TABLE meetings ADD COLUMN title TEXT`, and their existing
  meetings stay untitled. The title is local metadata, not part of the CRDT document, so it does
  not replicate to peers yet.

The `unassigned_voice_clips` table retains temporary review audio for weak or unknown speaker
matches, keyed by a UUID and its owning meeting. Each record includes the segment's start and end
millisecond offsets so a review client can seek to the relevant region without exposing audio to
application logs.

The `transcript_segments` table stores speaker-attributed transcript intervals: owning
`meeting_id`, `start_time_ms`/`end_time_ms` offsets from the meeting start, an optional
`speaker_identity_id` (a `SpeakerIdentity` UUID, `NULL` when unattributed), and `segment_text`.
`save_transcript_segments` replaces a meeting's rows in one transaction; `load_transcript_segments`
returns them ordered by start time. `load_meeting_metadata` exposes the meeting's `created_at_utc`
instant and `title` without reading transcript or CRDT payloads. Segment text follows the same at-rest policy
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
Embeddings are encrypted at rest (see below) and never logged.

## Meeting document as the source of truth (Issue #43)

When the composition roots open the repository with `with_document_codec` (via
`storage_composition::open_note_repository`), the meeting's Automerge document is authoritative for
its origin (`created_at`, `acquisition_device_id`), transcript, and segments. SQLite rows are an
index derived from it:

- `create_meeting`, `save_transcript`, and `save_transcript_segments` update the document and the
  rows in one transaction.
- `create_meeting` keeps the origin a peer's document already carries; a new local meeting gets this
  device and the current whole-second instant. A document that does not parse is rejected and no row
  is written.
- `save_crdt_document` (a merged document from a peer) projects it onto the index in the same
  transaction. Fields the document does not hold yet leave their rows untouched.
- Unknown-speaker clusters, chunk embeddings, and review clips stay local and are not in the document.

Databases from before this change are migrated once: every meeting's existing rows are written into
its document, and the `storage_migrations` table (`name` PK) records
`meeting_documents_v1` so later opens skip the scan. `sqlite_meeting_document_tests.rs` specifies
this behaviour. Without a codec (test fixtures only) the adapter stores documents as opaque bytes.

Every document write also sets `meetings.document_revision` to one past the newest stored revision
(indexed; older databases gain the column on open). `document_changes_since(revision)` lists the
meetings written after a revision by any connection or process. The daemon's P2P task uses it to
send new transcripts to connected devices about a second after they are saved.

The adapter rejects writes to unknown meetings rather than silently creating orphan notes. It emits structured `tracing` events with operation, device, and meeting identifiers only—never transcript content or CRDT bytes.

The `attachments` table persists only an attachment UUID, optional confirmed meeting ID, local
path, MIME type, original UTC timestamp, and source-device metadata. It never stores attachment
bytes. `AttachmentRepository` checks referenced meetings before accepting a confirmed link.

`sqlite_peer_store.rs` implements `NodeIdentityStore` (Issue #43) on the same SQLite file. The
singleton table `p2p_node_identity(singleton, sealed_identity)` holds this node's libp2p keypair,
sealed with `EmbeddingCipher::seal_secret` under the context `p2p_node_identity`. The candidate
is written with `INSERT OR IGNORE` and the stored row is read back, so processes starting together
agree on one identity. If the stored row can't be opened (wrong key), that's an error: replacing
it would orphan every paired device.
It also implements `TrustedPeerStore`. The table
`p2p_trusted_peers(peer_id PRIMARY KEY, device_name, paired_at_ms)` is the sync allowlist. Pairing
again upserts the name and time, and revoking deletes the row. Peer IDs and names are not
secret, so these rows are not sealed.

`sqlite_voice_vector_db.rs` implements `VoiceVectorDB` on the same SQLite file as the note
repository, through its own connection with a 5 s busy timeout:

| Table | Columns |
| --- | --- |
| `speaker_identities` | `id` (UUID PK), `display_name` (non-blank), `created_at`, `updated_at` (RFC 3339) |
| `voice_variants` | `id` (UUID PK), `identity_id` (FK → `speaker_identities`), `embedding` BLOB, `created_at` |
| `external_identity_mappings` | `external_id` (PK, e.g. a Teams user ID), `identity_id` (FK → `speaker_identities`), `created_at` (Issue #33) |

Saving an existing identity ID renames it. Identities list in creation order. `store_embedding`
rejects empty or non-finite vectors and any variant whose identity was never saved.
Each variant embedding is sealed with the injected cipher.
`search_similar` ranks every variant by cosine similarity inside the adapter. `delete_identity`
removes the variants and the name in one transaction. Both tables are created with
`IF NOT EXISTS` on every open, so databases from before Issue #30 migrate without touching
existing rows. The CLI, daemon, and FFI compose this store.

## Embedding encryption (Issue #31)

Both SQLite adapters take an `Arc<dyn EmbeddingCipher>` and seal every embedding before writing
it:

| Table | Column | Sealing context |
| --- | --- | --- |
| `voice_variants` | `embedding` | `voice_variants:<variant id>` |
| `unknown_speaker_clusters` | `representative_embedding` | `unknown_speaker_clusters:<cluster id>` |
| `cluster_chunks` | `derived_embedding` | `cluster_chunks:<cluster id>:<chunk id>` |
| `unassigned_voice_clips` | `derived_embedding` | `unassigned_voice_clips:<clip id>` |

`sqlite_embedding_encryption.rs` holds this table and the migration. Each table has an
`embedding_format` column: `0` is plaintext from before #31, `1` is sealed (AES-256-GCM, format
v1). Older databases gain the column with default `0`. Opening an adapter seals all format `0`
rows in one transaction per table; a row that already opens with the key is only re-marked, and
other formats are never touched. Reads always decrypt, so a plaintext or tampered value is an
error, never silently accepted. The adapters contain no cryptography; see
`core/application/crypto/README.md`.

`sqlite_encryption_metadata.rs` implements `EncryptionMetadataRepository` with the single-row
`encryption_meta` table: `key_source` (`key_file` or `passphrase`), `salt`,
`argon2_memory_kib`, `argon2_iterations`, `argon2_parallelism`, `key_check`, and `created_at`.
The row is inserted once (`ON CONFLICT DO NOTHING`) and never overwritten.

`open_in_memory(cipher)` exists for tests and ephemeral workflows; callers pass
`application::crypto::voice_encryptor::ephemeral_cipher()`, so even in-memory data is sealed.

`in_memory_voice_vector_db.rs` implements `VoiceVectorDB` with a process-local collection and a
simple cosine-similarity loop. It is suitable for deterministic unit tests and development only.
It does not provide persistence or AES-256-GCM encryption, and it accepts variants for unsaved
identities, so it must never be selected as the production biometric store.

`file_storage_adapter.rs` implements `FileStorage` as a SHA-256 content-addressed store. It writes
each verified object through a unique `.transfer-*.tmp` staging file and atomically renames it to
its hash path, so duplicate content consumes one local object and failed writes are cleaned up.

The same SQLite adapter implements `MetricsRepository`. Its `task_metrics` table stores only task
and device identifiers, model labels, timings, coarse power estimates, and timestamps; it contains
no recordings, transcript text, raw audio, or embeddings. `device_load_stats` stores the latest
non-sensitive CPU/GPU/task-count snapshot for each device. Both tables are exercised through the
adapter's in-memory SQLite constructor in unit tests.

`sqlite_external_identities.rs` (a child module of the voice store, Issue #33) implements
`ExternalIdentityRepository` for `SqliteVoiceVectorDB`. Saving a mapping also upserts the
identity row, and lookups join `speaker_identities`, so a rename in the voice library shows up
for later Teams meetings. `TeamsIdentityMapper` uses it through the port's `Mutex<_>` forwarding
impl, which the bridge pump thread needs. Only IDs and names are stored.

`slide_attachment_store.rs` (`SlideAttachmentStore`, Issue #33) implements `AttachmentWorkflow`
for Teams screen-share captures. Each PNG or JPEG (recognized by its signature) is written to
`<slide dir>/<meeting UUID>/slide-<ms>-<attachment UUID>.<ext>`, with directories 0700 and files
0600 on Unix, and linked to its meeting through `AttachmentRepository`. If the row is rejected
(unknown meeting), the file is deleted again. Slide files are not encrypted, the same policy as
manually added attachments.
It also implements `MeetingSlideSink` for local screen shares (Issue #49), which use a recorded
meeting's ID (`meeting-<uuid>`) as the folder name and the meeting time in milliseconds as
`<ms>`. A meeting ID that is not a plain folder name (anything besides ASCII letters, digits,
`-` and `_`) is rejected before anything is written, so no file can land outside the slide
directory.
