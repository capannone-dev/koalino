<!-- Explains host configuration types in the Teams bot core. -->

# Hosting

- `TeamsBotOptions`: the `TeamsBot` configuration section (WebSocket URL, shared secret, video
  FPS and quality, API key).
  - It maps onto the component options.
  - It refuses plaintext `ws://` to non-loopback hosts.
- `ApiKeyVerifier`: a constant-time check of the `X-Api-Key` header for the control endpoints.
  Keys shorter than 32 characters disable those endpoints.
