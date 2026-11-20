<!-- This directory provides the tracing-backed observability adapter for JSON console and rotating-file logs. -->

# Observability adapter

`../observability.rs` installs `tracing` output to stdout and to a daily rolling `app_log.<date>`
file in `NOTE_TAKER_LOG_DIR` (default `logs`), behind a reloadable filter (`LoggingRuntime::set_filter`)
so verbosity changes at runtime. The default level is `INFO`.

`structured_json.rs` formats both sinks. Each event is one JSON object with the fields required
by REQUIREMENTS.md 6.3:

```json
{"timestamp":"2026-10-28T09:05:00.123456Z","level":"INFO","target":"p2p_network",
 "device_id":"device-…","message":"synced","context":{"peer_id":"xyz","latency_ms":45}}
```

- `device_id` is on every line (it used to be on the start-up line only).
- Event fields go into `context` with their JSON types; `context` is omitted when there are none.
- **Redaction (6.4):** a field whose name contains `embedding`, `passphrase`, `password`,
  `secret`, `token`, `api_key`, `encryption_key`, `private_key`, `key_material`,
  `authorization`, `credential`, `title`, `transcript`, `raw_audio`, or `pcm`
  (case-insensitive) is written as `"[redacted]"`. Matching errs towards redacting. It is a
  safety net: code must still never log raw audio, embeddings, keys, or meeting content, and
  free-text messages are not inspected.

`structured_json_tests.rs` pins the required fields, typed context values, and redaction.

`log_export.rs` (`export_log_files`) joins the newest seven daily files, oldest first, into one
JSON-lines file for the app's Export logs and the daemon's log download (REQUIREMENTS 6.2,
Issue #46). It streams each file, ends a day cut off mid-line with a newline, writes through a
`.partial` file, and reports `NoLogs` when there is nothing to export. The entries were redacted
when written, so the export adds no user content. `log_export_tests.rs` covers the order, the
seven-day bound, other files left out, replacing an earlier export, and the empty case.

`log_retention.rs` (`prune_log_files`, `LogRetention`) removes old daily files when logging
starts (#147): it keeps the newest seven and, while they exceed 2 GiB, removes the oldest of them,
never the newest. The rolling appender is also built with `max_log_files(7)`, so a process left
running for days drops a day at each rotation. Other files are left alone.
`log_retention_tests.rs` covers the day count, the size limit, and a missing directory.

`live_log_control.rs` (`LiveLogControl`) implements the `LogControl` port over a filter reloader
built from the daemon's `LoggingRuntime`. The web Settings routes and the daemon's `cli log-level`
watcher both apply levels through it, so `GET /api/settings/log-level` shows the level in effect.
It validates like the control file, keeps the old level when the reload fails, and is tested with
a recording reloader (`live_log_control_tests.rs`).
