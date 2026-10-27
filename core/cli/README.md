<!-- Documents the command-line driving adapter and its stable command contract. -->

# Command-line adapter

This directory will contain the `clap` command model and service dispatch used by the Linux CLI.
It parses terminal input, delegates to application services and repository ports, and formats only
safe human-readable output. It must not access SQLite, perform identity/vector policy, or retain
raw audio itself.

## Commands

All commands open `NOTE_TAKER_DATABASE_PATH` (default `note_taker.sqlite3`), matching the daemon.
The executable intentionally fixes its tracing filter to `error`; terminal output therefore contains
only command results.

```text
note-taker record start|stop|status
note-taker meetings list
note-taker meetings view <meeting-id>
note-taker meetings export <meeting-id> --format <markdown|pdf|json> [--output <path>] [--no-attachments] [--no-metadata]
note-taker meetings export-all --format <markdown|pdf|json> [--date-range <YYYY-MM-DD>:<YYYY-MM-DD>] [--output <path>] [--no-attachments] [--no-metadata]
note-taker voice list
note-taker voice assign <clip-uuid> <identity-uuid>
note-taker telemetry
```

`meetings list` prints `ID | Date | Duration`; the current storage port exposes meeting IDs only,
so date and duration display as `unknown` until recording timeline metadata is added. `voice assign`
finds the clip through `NoteRepository` and delegates signature storage to `VoiceIdentityManager`;
it never performs vector matching or SQL in the CLI.

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
