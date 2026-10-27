<!-- Explains the integration tests. -->

# Integration tests

`BotToBridgeIntegrationTests` drives a mocked Teams meeting through the real `BotService`,
processors, and `WebSocketClient` into the in-process mock bridge server. It then decodes the
metadata, the tagged audio, and the screen-share slide that arrive there.
