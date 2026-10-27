<!-- Explains the bridge to the Rust app. -->

# Bridge

- `ProtobufSerializer`: builds and validates `TeamsBridgeMessage` frames (audio, video,
  metadata). The tests check its output byte for byte against the Rust golden fixtures.
- `WebSocketClient`: the connection to the Rust app's Teams bridge.
  - It sends `{"auth": secret}` and waits for `{"auth":"ok"}` before sending anything else.
  - It sends no `Origin` header.
  - It reconnects with exponential backoff and jitter, and re-sends the meeting metadata
    first.
  - It buffers up to 30 s of audio and 16 slides, dropping the oldest when full.
