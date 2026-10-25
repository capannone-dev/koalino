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
note-taker voice list
note-taker voice assign <clip-uuid> <identity-uuid>
note-taker telemetry
```

`meetings list` prints `ID | Date | Duration`; the current storage port exposes meeting IDs only,
so date and duration display as `unknown` until recording timeline metadata is added. `voice assign`
finds the clip through `NoteRepository` and delegates signature storage to `VoiceIdentityManager`;
it never performs vector matching or SQL in the CLI.
