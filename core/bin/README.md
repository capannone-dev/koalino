<!-- This directory contains executable composition roots that wire ports to adapters at runtime. -->

# Binary entry points

`cli.rs` is the command-line composition root and `daemon.rs` is the headless service composition root. They own process-level configuration such as logging. `ffi.rs` owns the C-compatible Flutter boundary: it exposes primitive status-code functions only, keeping Rust domain structs and adapter implementations out of Dart.

Attachment import uses a caller-freed JSON string for ranked metadata only. `ffi.rs` composes
`AttachmentManager` with the local metadata and SQLite adapters, then receives opaque IDs back for
the user-confirmed persistence action; it never accepts attachment bytes from Flutter.

For background services and lock-screen controls, `ffi.rs` owns its recorder in a process-wide
`OnceLock<Mutex<...>>`; no FFI caller receives or retains a Rust pointer. Start, Pause, Stop,
interruption, and state-query commands take only this mutex and operate on the portable
`AudioCapture` state machine. State query codes are deliberately `10` through `15`, leaving the
documented nonzero status codes distinguishable from lifecycle states.

`daemon.rs` composes the headless Axum server from `SqliteNoteRepository` and a local
`VoiceVectorDB`, passing them only through `ServerAppState`. HTTP route handlers never use SQLite
directly; they consume repository ports. The daemon defaults to loopback binding and must sit
behind an authenticated reverse proxy before any non-loopback deployment.

`cli.rs` is the interactive terminal composition root. It parses the `core/cli` clap model, opens
the same `NOTE_TAKER_DATABASE_PATH` as the daemon, and passes portable adapters to the thin CLI
service. It fixes structured log filtering at `error` so a user's table/detail output is not mixed
with operational logs.
