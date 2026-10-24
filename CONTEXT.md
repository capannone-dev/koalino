<!-- Captures the current implementation boundary for contributors resuming work on the project. -->

# Engineering context

Issue #1 establishes only the Rust hexagonal scaffold. `core/domain` is pure business code; `core/ports` provides interfaces; `core/application` coordinates ports; `core/adapters/observability` configures tracing. AI, libp2p, SQLite, CRDT, Flutter, and server adapters are deliberately not implemented or depended upon yet.

Logging emits JSON to stdout and a daily `app_log` rolling file. The `LoggingRuntime` provides a reloadable filter for a future UI or CLI runtime-control surface. Events must include `device_id` and must never log raw audio, voice embeddings, E2EE keys, or other sensitive values.
