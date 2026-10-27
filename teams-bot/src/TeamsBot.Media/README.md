<!-- Explains the Teams/Graph media adapter project. -->

# TeamsBot.Media

The adapter between Microsoft Teams and the core abstractions. It is built on the Graph
Communications calling SDK with application-hosted Real-Time Media
(`Microsoft.Graph.Communications.Calls.Media`, which brings in `Microsoft.Skype.Bots.Media`).
It compiles anywhere but runs only on Windows x64.

- `GraphCallJoiner`:
  - Builds the communications client with MSAL tokens and the 401 retry handler. The SDK's
    built-in provider validates inbound callback JWTs.
  - Initializes the media platform.
  - Joins meetings with a receive-only media session: unmixed 16 kHz audio and VBSS in NV12.
  - Relays `/api/calling` callbacks into the SDK.
- `GraphCallSession`: maps Graph roster updates, unmixed audio buffers, VBSS frames, and call
  termination onto `ICallSession`.
  - It subscribes to the current screen sharer.
  - It copies native buffers and always disposes them.
- `ParticipantMapping`: Graph participant → Teams user ID (Azure AD object ID, else
  `teams-participant-<id>`), display name, and audio and VBSS source IDs.
- `GraphTokenProvider` / `MsalTokenSource`: client-credentials tokens for the SDK.
- `GraphBotOptions.cs`: the `AzureAd` and `TeamsBot:Calling` configuration sections.
