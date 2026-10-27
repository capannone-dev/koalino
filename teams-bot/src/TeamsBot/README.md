<!-- Explains the ASP.NET Core host project of the Teams bot. -->

# TeamsBot (host)

The executable bot.

- `Program.cs`: binds the configuration, configures Serilog, registers the services, and stops
  at startup when the configuration is invalid.
- `HostConfiguration.cs`: validates all configuration sections together.
- `BotEndpoints.cs`: the HTTP endpoints.
  - `/api/calling` relays Graph callbacks into the SDK.
  - `/api/join` and `/api/leave` are protected by `X-Api-Key`.
  - `/healthz` reports the bridge and meeting state without identifiers.
- `BotLifetimeService.cs`: starts the WebSocket client with the host. On shutdown it leaves the
  meeting and drains the bridge.
- `appsettings.json`: defaults only. Secrets come from environment variables or user secrets.

See the bot's [README](../../README.md#7-configuration) for every setting.
