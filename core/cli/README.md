<!-- Documents the command-line driving adapter and its stable command contract. -->

# Command-line adapter

This directory will contain the `clap` command model and service dispatch used by the Linux CLI.
It parses terminal input, delegates to application services and repository ports, and formats only
safe human-readable output. It must not access SQLite, perform identity/vector policy, or retain
raw audio itself.

## Commands

All commands open `NOTE_TAKER_DATABASE_PATH` (default `note_taker.sqlite3`), matching the daemon.
Voice embeddings in it are encrypted. Set `NOTE_TAKER_ENCRYPTION_PASSPHRASE` to derive the key
from a passphrase; otherwise a random key is created in `~/.note_taker/encryption.key` (or
`NOTE_TAKER_ENCRYPTION_KEY_DIR`). Keep using the same choice: a database encrypted one way refuses
to open the other way, and a lost passphrase or key file makes its voice data unrecoverable.
The executable intentionally fixes its tracing filter to `error`; terminal output therefore contains
only command results.

```text
note-taker record start --input <file.wav|-> [--meeting-id <id>]
note-taker record stop|status
note-taker meetings list
note-taker meetings view <meeting-id>
note-taker meetings export <meeting-id> --format <markdown|pdf|json> [--output <path>] [--no-attachments] [--no-metadata]
note-taker meetings export-all --format <markdown|pdf|json> [--date-range <YYYY-MM-DD>:<YYYY-MM-DD>] [--output <path>] [--no-attachments] [--no-metadata]
note-taker voice list
note-taker voice assign <clip-uuid> <identity-uuid>
note-taker voice list-clusters <meeting-id>
note-taker voice assign-cluster <cluster-uuid> <identity-name>
note-taker telemetry
```

`meetings list` prints `ID | Date | Duration`; the current storage port exposes meeting IDs only,
so date and duration display as `unknown` until recording timeline metadata is added. `voice assign`
finds the clip through `NoteRepository` and delegates signature storage to `VoiceIdentityManager`;
it never performs vector matching or SQL in the CLI.

### Unknown-speaker clusters

Both commands call `application::cluster_review`, the same use case as the HTTP API and FFI.

`voice list-clusters <meeting-id>` prints the meeting's unresolved clusters. Durations are in
seconds, rounded to one decimal place:

```text
Cluster ID | Total Duration | Chunks
4b1c…-… | 12.5s | 3
```

A meeting with no unresolved clusters, including an unknown meeting ID, prints
`No unresolved unknown-speaker clusters for meeting <meeting-id>`.

`voice assign-cluster <cluster-uuid> "<identity name>"` resolves the name and assigns the cluster.
A known identity with the same name (trimmed, case-insensitive) is reused. Any other name gets a
new identity UUID. Every member embedding is stored as a voice variant, the cluster's transcript
segments are relinked, the cluster is resolved, and its review audio is deleted. It prints:

```text
Assigned cluster <cluster-uuid> to "Grace Hopper" (new identity <identity-uuid>; 3 voice variants stored, 4 transcript segments updated)
```

Blank names, unknown clusters, and already-resolved clusters exit non-zero with a readable message.

The CLI binary composes `SqliteVoiceVectorDB` on `NOTE_TAKER_DATABASE_PATH`, so the name and
the voice variants persist with the relinked segments and resolved cluster. A later
`voice list` shows the identity, and a later `voice assign-cluster` with the same name in any
letter case reuses it.

### Exports

`meetings export` and `meetings export-all` call the same `ExportManager` that the HTTP API and
the Flutter FFI use. The binary composes an `ExportWriter` with every exporter from
`adapters::export::default_exporters()` and the directory from `NOTE_TAKER_EXPORT_DIR` (default
`~/Documents/NoteTaker/exports`).

- `--format` accepts `markdown` (or `md`), `pdf`, and `json`, case-insensitively. `srt` is reserved
  and rejected with `srt export is not supported yet`.
- `--output` writes to an explicit file. Without it, a unique name such as
  `2026-10-27_<meeting-id>_20261027T091500123Z.md` is generated in the export directory.
- `--date-range 2026-10-01:2026-10-31` selects meetings created between the start day's midnight
  and the end day's last millisecond (UTC, inclusive). Without a range, `export-all` exports
  every meeting into one file.
- Markdown exports copy attachments into `<file stem>_attachments/` beside the file and link them
  relatively. PDF exports embed them. JSON exports list their original paths.

On success the CLI prints `Exported <n> meeting(s) as <format> to <path> (<bytes> bytes)`. Unknown
meetings, empty selections, and invalid flags exit non-zero with a readable message.

## Recording (Issue #32)

`record start` runs in the foreground. It composes the meeting pipeline (see
`core/bin/README.md` for the transcription variables), feeds 16 kHz mono 16-bit PCM from a WAV
file or, with `--input -`, raw little-endian PCM from stdin, and prints a summary once every
chunk is transcribed. A microphone can be recorded on Linux without extra dependencies:

```text
arecord -f S16_LE -r 16000 -c 1 -t raw | note-taker record start --input - --meeting-id standup
```

The recording ends when the input ends, on Ctrl+C, or when another terminal runs `record stop`.
`record stop` waits until the recording has transcribed its remaining audio. `record status`
prints `Recording (meeting …, pid …)` or `Idle`. Without a transcription engine, `record start`
fails with the variables to set. The stop request is noticed between 100 ms input blocks, so a
silent stdin pipe that delivers no data delays it.
