<!-- Explains the C# Teams bot: what it does, how to register, configure, build, run, deploy, and troubleshoot it. -->

# Koalino Teams bot

A C#/.NET 8 bot that joins Microsoft Teams meetings, captures each participant's audio and
the shared screen, and streams them to the Rust Koalino app. The Rust app then transcribes
locally, with speaker attribution. The bot implements Issue #34; the Rust side of the protocol
is Issue #33.

What the bot sends:

- **Audio:** PCM, 16 kHz, mono, signed 16-bit little-endian. Every chunk is tagged with the
  speaker's Teams user ID (Azure AD object ID) and display name.
- **Slides:** screen-share frames sampled at `TeamsBot:VideoFps` (default 1 per second),
  encoded as JPEG at `TeamsBot:VideoQuality` (default 80), and sent as
  `VideoFrame { source: "screen_share" }`. Unchanged frames are skipped.
- **Meeting metadata:** the meeting ID, title, start time, and roster. It is sent first and
  sent again after every reconnect and roster change.

## 1. Architecture

```text
 Microsoft Teams meeting
   │  signaling: HTTPS webhook  POST /api/calling   (JWT-signed by the calling service)
   │  media:     TCP 8445 (SRTP)  unmixed audio per speaker + VBSS screen share (NV12)
   ▼
 ┌─────────────────────────── Teams bot (Windows Server, this folder) ───────────────────────────┐
 │ src/TeamsBot (ASP.NET Core host, Serilog)                                                     │
 │   /api/calling ─▶ GraphCallJoiner (Graph Communications SDK + Real-Time Media)                 │
 │   /api/join    ─▶ BotService.JoinMeetingAsync ─▶ updateRecordingStatus(recording)             │
 │                                                                                               │
 │ src/TeamsBot.Media   GraphCallSession: roster → MSI→participant map, audio/VBSS → managed     │
 │ src/TeamsBot.Core    BotService ─┬─▶ AudioProcessor  (→16 kHz mono, per-speaker 1–10 s chunks)│
 │                                  └─▶ VideoProcessor  (1 fps, NV12→JPEG q80, skip unchanged)   │
 │                      WebSocketClient (auth, reconnect 1→30 s backoff, 30 s buffer)            │
 │                      ProtobufSerializer (core/protocol/teams_bridge.proto)                    │
 └───────────────────────────────────────┬───────────────────────────────────────────────────────┘
                                         │ ws://127.0.0.1:8765  (same host, or SSH tunnel)
                                         │ {"auth":"<secret>"} → {"auth":"ok"}, then binary frames
                                         ▼
 Rust Koalino daemon: TeamsWebSocketServer → transcription → speaker-attributed notes + slides
```

The Rust server binds `127.0.0.1` only. Run the bot on the same machine, or carry the
connection over an SSH tunnel (see section 8). The bot refuses plaintext `ws://` to any
non-loopback host.

### Projects

| Path | Purpose | Runs on |
| --- | --- | --- |
| `src/TeamsBot.Core` | Audio/video processing, Protobuf serializer, WebSocket client, meeting lifecycle (`BotService`), token cache, join-link parser, config model. No Teams SDK. | Any OS |
| `src/TeamsBot.Media` | Adapter to the Graph Communications calling SDK and Real-Time Media (`GraphCallJoiner`, `GraphCallSession`, MSAL). | Builds anywhere; runs on Windows x64 |
| `src/TeamsBot` | ASP.NET Core host: configuration, Serilog, HTTP endpoints, startup/shutdown. | Windows x64 |
| `tests/TeamsBot.Core.Tests` | xUnit + NSubstitute: golden Protobuf bytes, audio/video, WebSocket client against a mock server, lifecycle, integration. | Any OS |
| `deploy/` | Windows container image and the Teams app manifest template. | |

## 2. Prerequisites

- **.NET 8 SDK** (8.0.100 or later; see `global.json`). Running the tests also needs the
  ASP.NET Core 8 runtime, because the mock bridge server uses Kestrel. On Arch:
  `pacman -S dotnet-sdk-8.0 aspnet-runtime-8.0`.
- **To run the bot against Teams:**
  - Windows Server 2019/2022 or Windows 10/11, x64. Real-Time Media is Windows-only.
  - A public DNS name and a TLS certificate for it.
  - A public IP address and open ports: 443 (signaling) and 8445/TCP (media).
- **Accounts:** an Azure subscription and a Microsoft 365 tenant where you are, or can ask, a
  Global/Teams administrator, who must grant admin consent.

## 3. Azure AD app registration

1. Open **Azure portal → Microsoft Entra ID → App registrations → New registration**.
   - Name: `Koalino Bot`. Supported account types: **single tenant**.
   - No redirect URI. Create it.
   - Copy the **Application (client) ID** into `AzureAd:ClientId` and the **Directory
     (tenant) ID** into `AzureAd:TenantId`.
2. Go to **Certificates & secrets → New client secret**. Copy the value immediately and store
   it in `AzureAd__ClientSecret`; never commit it. Rotate it before it expires (section 12
   covers the symptom).
3. Go to **API permissions → Add a permission → Microsoft Graph → Application permissions** and
   add:

   | Permission | Why |
   | --- | --- |
   | `Calls.JoinGroupCall.All` | Join meetings and group calls. |
   | `Calls.Initiate.All` | Create the outgoing call that joins the meeting. |
   | `Calls.AccessMedia.All` | Receive the call's audio and video (app-hosted media). |

   Then select **Grant admin consent for <tenant>**. Without consent, joins fail with 403.
4. The bot authenticates with MSAL client credentials (scope
   `https://graph.microsoft.com/.default`).
   - Tokens are cached and refreshed 5 minutes before they expire.
   - A Graph `401` invalidates the cached token and retries the request once with a fresh one.
   - Failed acquisitions are retried with backoff.
   - Every inbound `/api/calling` callback must carry a JWT signed by the Teams calling
     service and addressed to this app (validated by the SDK's built-in provider), or it gets
     `401`.

## 4. Azure Bot resource and calling webhook

1. Go to **Azure portal → Create a resource → Azure Bot**.
   - Type of app: **Single tenant**. Choose **Use existing app registration** and enter the
     client ID from step 3.
2. Open the bot, then **Channels → Microsoft Teams**.
   - Accept the terms. On the **Calling** tab, tick **Enable calling**.
   - Set **Webhook (for calling)** to `https://<your-fqdn>/api/calling`.
   - The same URL goes into `TeamsBot:Calling:NotificationUrl`. Save.

## 5. Teams app package

Calling bots must be installed as a Teams app that declares `supportsCalling` and
`supportsVideo`. Fill in `deploy/teams-app/manifest.json` and zip it (see
[deploy/teams-app/README.md](deploy/teams-app/README.md)). Then upload it in Teams, or publish
it from the Teams admin center. Custom-app upload must be allowed by your Teams app setup
policy.

## 6. Media hosting: DNS, certificate, ports

- **DNS:** an A record such as `bot.example.com` pointing at the host's public IP. This is
  `TeamsBot:Calling:ServiceFqdn`.
- **Certificate:** a publicly trusted certificate for that name, installed in
  `LocalMachine\My` with its private key. Give the bot's account read access to the key. Put
  the thumbprint in `TeamsBot:Calling:CertificateThumbprint`. The media platform uses it for
  the media TLS handshake.
- **Ports:**
  - 443 → the host's signaling port, 9441 by default (`Urls`), through a TLS-terminating
    reverse proxy (IIS, nginx, Azure Application Gateway) or Kestrel HTTPS.
  - 8445/TCP for media, inbound (`MediaPublicPort` / `MediaInternalPort`).
  - Open both in the VM's network security group and in Windows Firewall.

## 7. Configuration

All settings live in `src/TeamsBot/appsettings.json`. Any of them can be overridden with an
environment variable that uses `__` for `:` (for example `TeamsBot__SharedSecret`). Keep
secrets out of files: use environment variables, `dotnet user-secrets` (development), or Azure
Key Vault.

| Key | Default | Meaning |
| --- | --- | --- |
| `AzureAd:TenantId` | — (required) | Directory (tenant) ID. |
| `AzureAd:ClientId` | — (required) | Application (client) ID, which is also the Azure Bot's app ID. |
| `AzureAd:ClientSecret` | — (required, secret) | Client secret. |
| `TeamsBot:WebSocketUrl` | `ws://localhost:8765` | Rust bridge endpoint. `ws://` is accepted only for loopback; otherwise use `wss://`. |
| `TeamsBot:SharedSecret` | empty (secret) | Must equal `NOTE_TAKER_TEAMS_BRIDGE_SECRET` on the Rust side. |
| `TeamsBot:VideoFps` | `1` | Screen-share frames sampled per second (0 < fps ≤ 30). |
| `TeamsBot:VideoQuality` | `80` | JPEG quality (1–100). |
| `TeamsBot:ApiKey` | empty (secret) | `X-Api-Key` for `/api/join` and `/api/leave`. At least 32 characters; when empty, the endpoints are disabled. |
| `TeamsBot:Calling:NotificationUrl` | — (required) | Public `https://…/api/calling`. |
| `TeamsBot:Calling:ServiceFqdn` | — (required) | Public DNS name; must match the certificate. |
| `TeamsBot:Calling:CertificateThumbprint` | — (required) | Media certificate in `LocalMachine\My`. |
| `TeamsBot:Calling:MediaPublicPort` / `MediaInternalPort` | `8445` | Media TCP port, public and local. |
| `TeamsBot:Calling:MediaPublicIpAddress` | resolved from the FQDN | Public IPv4 address, if DNS resolution is not wanted. |
| `TeamsBot:Calling:JoinTimeout` | `00:01:30` | How long a join may take, including time in the lobby. |
| `Serilog:*` | console, Information | Standard `Serilog.Settings.Configuration` keys. |

The host validates every setting at startup and exits with one `Configuration error: …` line
per problem. The error lists the setting's name, never its value.

Generate the API key and the shared secret with
`openssl rand -base64 48`, or `[Convert]::ToBase64String((1..48|%{Get-Random -Max 256}))` in
PowerShell.

## 8. Connecting to the Rust app

Start the Rust daemon with the bridge enabled. Choose your own secret; the bot's
`TeamsBot__SharedSecret` must be the same value.

```bash
export NOTE_TAKER_TEAMS_BRIDGE_ENABLED=true
export NOTE_TAKER_TEAMS_BRIDGE_PORT=8765
export NOTE_TAKER_TEAMS_BRIDGE_SECRET='<same value as TeamsBot__SharedSecret>'
cargo run --release --bin daemon
```

- **Same host:** keep `TeamsBot:WebSocketUrl = ws://localhost:8765`.
- **Different hosts:** the Rust app usually runs on a laptop behind NAT, while the bot needs a
  public Windows server. Open an **SSH reverse tunnel from the Rust machine** to the bot host.
  The bot still connects to its own loopback, and the traffic crosses the network encrypted by
  SSH:

  ```bash
  ssh -N -R 127.0.0.1:8765:127.0.0.1:8765 user@bot.example.com
  ```

  Windows ships OpenSSH Server as an optional feature. If the bot host can reach the Rust
  machine instead, a forward tunnel works too (`ssh -N -L 8765:127.0.0.1:8765 user@rust-host`
  run on the bot host).

## 9. Build, test, run

```bash
cd teams-bot
dotnet build TeamsBot.sln            # warnings are errors; missing XML docs fail the build
dotnet test TeamsBot.sln             # 94 tests; Teams SDK mocked, mock Rust server in-process
```

- `ProtobufSerializerTests` compares the bot's bytes with the Rust golden fixtures in
  `core/protocol/fixtures/`, so the two sides cannot drift.
- `BotToBridgeIntegrationTests` drives a mocked meeting through the real processors and
  WebSocket client into a mock bridge server.

Run the bot on Windows:

```powershell
cd teams-bot\src\TeamsBot
dotnet user-secrets set "AzureAd:ClientSecret" "<secret>"
dotnet user-secrets set "TeamsBot:SharedSecret" "<shared secret>"
dotnet user-secrets set "TeamsBot:ApiKey" "<api key>"
dotnet run --configuration Release
```

Join a meeting by posting its **Join** link. The endpoint returns `202` and the bot joins in the
background. Watch the log, or poll `GET /healthz`:

```bash
curl -X POST https://bot.example.com/api/join \
  -H "X-Api-Key: $TEAMS_BOT_API_KEY" -H "Content-Type: application/json" \
  -d '{"joinUrl":"https://teams.microsoft.com/l/meetup-join/19%3ameeting_…%40thread.v2/0?context=%7b%22Tid%22%3a%22…%22%2c%22Oid%22%3a%22…%22%7d"}'
curl -X POST https://bot.example.com/api/leave -H "X-Api-Key: $TEAMS_BOT_API_KEY"
curl https://bot.example.com/healthz     # {"bridgeConnected":true,"inMeeting":true,"bufferedAudioMs":0}
```

Accepted link forms:

- the classic `…/l/meetup-join/<thread>/<message>?context={"Tid":…,"Oid":…}`;
- the short `…/meet/<meeting id>?p=<passcode>`.

Only Teams hosts are accepted over https: `teams.microsoft.com` and the US government clouds.

## 10. Deployment

### Local development (Windows workstation + ngrok)

Teams must reach both the webhook and the media port.

1. Signaling: `ngrok http 9441` gives a public https URL. Use
   `<ngrok-url>/api/calling` as the Azure Bot calling webhook and as
   `TeamsBot:Calling:NotificationUrl`.
2. Media: `ngrok tcp 8445` gives an address like `0.tcp.ngrok.io:12345`. Use a reserved TCP
   address so it stays the same across restarts.
   - Create a CNAME such as `0.bot.example.com` pointing at it.
   - Get a certificate for that name and set `ServiceFqdn` to it.
   - Set `MediaPublicPort` to the ngrok port (12345) and `MediaInternalPort` to `8445`.
3. Run the Rust daemon on the same workstation (WSL works) and the bot with `dotnet run`.

### Docker (Windows containers)

Docker is only usable with Windows containers, because the media platform does not run on
Linux.

```powershell
docker build -f teams-bot/deploy/Dockerfile -t note-taker-teams-bot .
docker run -d --name teams-bot -p 9441:9441 -p 8445:8445 `
  -e AzureAd__TenantId=... -e AzureAd__ClientId=... -e AzureAd__ClientSecret=... `
  -e TeamsBot__SharedSecret=... -e TeamsBot__ApiKey=... `
  -e TeamsBot__WebSocketUrl=wss://bridge.example.com:8765 `
  -e TeamsBot__Calling__NotificationUrl=https://bot.example.com/api/calling `
  -e TeamsBot__Calling__ServiceFqdn=bot.example.com `
  -e TeamsBot__Calling__CertificateThumbprint=... note-taker-teams-bot
```

- **Certificate:** import it into the container's `LocalMachine\My` store, for example with a
  derived image that runs `Import-PfxCertificate` at build time from a build secret, or with
  gMSA/Key Vault tooling.
- **Reaching the Rust app:** a container cannot reach the host's loopback, so plaintext
  `ws://localhost` is not an option.
  - Either terminate TLS in front of an SSH tunnel and use `wss://`,
  - or run the bot directly on the host instead of in a container.

### Azure VM (recommended for production)

1. Create a **Windows Server 2022** VM (D4s_v5 or larger; media needs ≥ 2 vCPUs per
   concurrent call) with a static public IP and a DNS label.
2. In the NSG, allow inbound 443/TCP and 8445/TCP.
3. Install the .NET 8 Hosting Bundle.
4. Install the certificate into `LocalMachine\My`.
5. Run `dotnet publish src/TeamsBot -c Release -o C:\bot` and install it as a Windows service
   (`sc.exe create NoteTakerBot binPath= "C:\bot\TeamsBot.exe"`), or host it behind IIS for the
   443 TLS termination.
6. Keep the secrets in Azure Key Vault or in machine-level environment variables.
7. Start the SSH reverse tunnel from the Rust machine (section 8).

## 11. Behaviour and resilience

- **Joining:**
  - The bot calls Graph `updateRecordingStatus(recording)` before it processes any media, so
    participants see the recording banner.
  - If Teams refuses, the bot hangs up without capturing anything.
  - Join failures (expired token, transient Graph errors) are retried 3 times with backoff.
  - Waiting in the lobby counts toward `JoinTimeout`.
- **Roster:**
  - Each join or leave is logged with the participant's Teams user ID; display names are not
    logged.
  - The meeting metadata is re-sent with the updated roster. The Rust app resumes a meeting it
    already knows, so this only refreshes identities.
  - Unknown audio sources get a placeholder identity `teams-media-source-<id>`, with a
    warning.
- **Audio:**
  - Teams delivers unmixed 20 ms buffers for up to four dominant speakers.
  - They are normalized to 16 kHz mono. Each speaker's audio becomes a chunk that ends after
    600 ms of silence or at 10 s. The protocol cap is 30 s.
  - Timestamps are milliseconds since the bot joined (`MeetingMetadata.start_time_ms`).
- **Video:**
  - VBSS NV12 frames are converted with BT.601 and handled at any stride and on resolution
    changes.
  - Frames are sampled at `VideoFps`, skipped when unchanged, and encoded as JPEG.
  - An oversized frame is re-encoded at a lower quality, then downscaled, so it stays under the
    8 MiB message cap.
- **WebSocket:**
  - The client sends the auth frame first, and sends no `Origin` header (the Rust server
    rejects one).
  - Reconnects back off exponentially from 1 s to 30 s with ±20 % jitter. A rejected secret
    (close 1008) is logged as an error and retried at the 30 s cap.
  - While disconnected, up to 30 s of audio and 16 slides are buffered. The oldest are dropped
    first, with a warning every 5 s at most.
  - After every reconnect, metadata is sent before any media.
- **Ending:**
  - When the organizer ends the meeting or the bot is removed, pending audio is flushed and the
    bridge is drained for up to 10 s. Late media events are ignored.
  - Shutting down the host (Ctrl+C, service stop) leaves the meeting cleanly.
- **Never crash:** every Teams SDK callback catches and logs its own errors and always frees
  native media buffers.
- **Known limitation:**
  - The protocol has no acknowledgements. A frame sent just before a connection dies may be
    lost, or delivered twice if it is re-sent after the reconnect.
  - Meeting metadata is idempotent on the Rust side, so in practice only one audio chunk at a
    disconnect is at risk.

## 12. Privacy and security

- **Encryption:** media from Teams is SRTP-encrypted. The bridge stays on loopback, or uses an
  SSH or TLS tunnel; plaintext `ws://` to a remote host is refused at startup.
- **Logging:** logs contain IDs, counts, and error types only. They never contain audio, images,
  meeting titles, e-mail addresses, display names, join links, tokens, or secrets. Graph SDK
  messages are not forwarded because they may contain names.
- **No storage:** the bot keeps no audio on disk. Buffers live in memory until they are sent.
  The Rust app deletes raw audio after transcription.
- **Join endpoint:** `/api/join` requires a ≥ 32-character API key, compared in constant time,
  and accepts only Teams meeting links.

## 13. Manual test in a real meeting

1. Start the Rust daemon with the bridge enabled, then the bot.
   - `GET /healthz` shows `"bridgeConnected": true`.
   - The bot logs `Connected to the Rust Teams bridge`.
2. Schedule a Teams meeting with two other people. Copy the **Join** link and `POST /api/join`.
3. Expected:
   - The bot appears as a participant.
   - Teams shows "Recording has started".
   - The log shows `Joined meeting … recording announced`.
4. Have each person speak in turn for about 10 s. The daemon logs Teams chunks, and the notes
   show `Known` segments attributed to the right names.
5. Share a slide deck and change slides a few times. The meeting's attachments gain one JPEG
   per distinct slide, at about 1 fps.
6. Stop the Rust daemon for about 20 s while people keep talking, then restart it.
   - The bot logs `disconnected`, then reconnects.
   - The buffered audio arrives; drop warnings appear only beyond 30 s.
7. Let a participant leave and rejoin. Their leave and join are logged with their Teams user
   ID.
8. End the meeting for everyone. The log shows `Meeting … ended unexpectedly: call terminated …`.
   Pending audio is flushed and the bot is ready for the next `POST /api/join`.

## 14. Troubleshooting

| Symptom | Likely cause and fix |
| --- | --- |
| `Configuration error: …` at startup | A required key is missing or unsafe; the message names it. |
| `Graph access token acquisition attempt … failed (MsalServiceException)` | Wrong tenant, client ID, or secret, or the secret expired. Create a new secret. |
| Join fails with 403 / `Join attempt … failed` | Admin consent is missing for `Calls.*` permissions, or the Teams app is not installed or allowed. |
| Join times out | The bot is waiting in the lobby: admit it, or set the meeting lobby to "Everyone". Or the webhook is unreachable: check that the Azure Bot calling webhook equals `NotificationUrl` and is reachable over https. |
| `/api/calling` answers 401 in the request log | The request did not come from Teams, or `AzureAd:ClientId` does not match the Azure Bot's app ID. |
| The call connects, then drops after a few seconds with no audio | The media port is unreachable, or the certificate does not match `ServiceFqdn`, or its private key is not readable. Check the NSG, the firewall, and the thumbprint. |
| `Teams refused the recording status` | The tenant's policy blocks the bot, or the permissions are missing. The bot leaves on purpose rather than record without the notice. |
| `rejected the shared secret` | `TeamsBot:SharedSecret` differs from `NOTE_TAKER_TEAMS_BRIDGE_SECRET`. |
| `bridgeConnected: false` and repeated `disconnected` warnings | The Rust daemon is not running, the bridge is not enabled, or the SSH tunnel is down. Check `ss -ltn | grep 8765` on both ends. |
| `Teams bridge buffer full; dropped the oldest … frames` | The Rust app was unreachable for more than 30 s, or transcription is slower than real time. |
| Audio attributed to `Unknown participant N` | The roster has not delivered that speaker's media source yet. It normally resolves within seconds. |
| No slides | Nobody is sharing, or the share is unchanged (frames are skipped), or the share is a PowerPoint Live presentation (not VBSS). |
| `DllNotFoundException` / media platform fails to initialize | The bot is running on Linux, macOS, or 32-bit. Run it on Windows x64 with the Visual C++ runtime installed. |
