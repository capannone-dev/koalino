<!-- Explains the layout of the core unit and integration tests. -->

# TeamsBot.Core.Tests

| Folder | What it specifies |
| --- | --- |
| `Bridge/` | Golden Protobuf bytes against `core/protocol/fixtures/*.bin`. The WebSocket client against the mock server: auth, 1008, reconnect, metadata resend, the 30 s buffer, backoff. |
| `Audio/` | Resampling and downmixing, per-speaker chunking, the silence gap, the maximum length, unknown speakers. |
| `Video/` | NV12 colours and stride, throttling, unchanged-frame skipping, resolution changes, oversize handling. |
| `Meeting/` | `BotService` lifecycle with NSubstitute mocks, and join-link parsing. |
| `Authentication/` | The token cache and the 401 retry handler. |
| `Hosting/` | Configuration defaults and validation, and the API-key check. |
| `Integration/` | A mocked meeting through the real processors and WebSocket client into the mock server. |
| `Support/` | Recording loggers and the mock bridge server. |

The fixtures are linked from `core/protocol/fixtures/`, so a schema change on either side fails
here.
