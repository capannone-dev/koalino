<!-- Explains the portable core library of the Teams bot. -->

# TeamsBot.Core

Platform-neutral bot logic. It builds and is tested on any OS, and has no dependency on the
Teams SDK.

| Folder | Contents |
| --- | --- |
| `Model/` | Bridge records: participant, meeting, PCM chunk, encoded frame. |
| `Abstractions/` | Seams to the Teams adapter (`ICallJoiner`, `ICallSession`, media frames, roster) and to the bridge (`IBridgeSink`). |
| `Audio/` | `PcmConverter` (to 16 kHz mono) and `AudioProcessor` (per-speaker chunking). |
| `Video/` | `VideoProcessor` (sampling, change detection, NV12→JPEG with SkiaSharp). |
| `Bridge/` | `ProtobufSerializer` and `WebSocketClient` (auth, reconnect, buffering). |
| `Meeting/` | `BotService` (lifecycle), `RosterDirectory`, `TeamsJoinUrl`. |
| `Authentication/` | Graph token cache and the 401 retry handler. |
| `Hosting/` | The `TeamsBot` configuration model and the API-key verifier. |

The C# Protobuf types (`NoteTaker.TeamsBridge.V1`) are generated at build time from
`../../../core/protocol/teams_bridge.proto`. That file is the single schema shared with the
Rust app; never copy it.
