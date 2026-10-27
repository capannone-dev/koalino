<!-- Explains the source projects of the Teams bot and how they depend on each other. -->

# Source projects

The dependencies run in one direction: `TeamsBot` → `TeamsBot.Media` → `TeamsBot.Core`.

- `TeamsBot.Core/`: portable logic with no Teams SDK. It holds audio and video processing,
  the Protobuf serializer, the WebSocket bridge client, the meeting lifecycle, and the
  configuration model. This is where the unit tests focus.
- `TeamsBot.Media/`: the adapter to the Microsoft Graph Communications calling SDK and
  Real-Time Media. It compiles on any OS but runs only on Windows x64.
- `TeamsBot/`: the ASP.NET Core host. It wires the services together and exposes
  `/api/calling`, `/api/join`, `/api/leave`, and `/healthz`.
