<!-- Explains shared test helpers. -->

# Test support

- `RecordingLogger<T>` / `RecordingLoggerFactory`: capture log entries, so tests can assert on
  warnings and check that secrets never appear in logs.
- `MockBridgeServer`: an in-process Kestrel WebSocket server that behaves like the Rust bridge.
  It handles shared-secret auth or a 1008 close, refused connections, and forced drops, and it
  records the frames it receives.
