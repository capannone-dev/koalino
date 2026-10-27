<!-- Explains the Teams bot test projects. -->

# Tests

`TeamsBot.Core.Tests/` (xUnit + NSubstitute) covers the portable core. The Teams SDK is mocked
behind `ICallJoiner` and `ICallSession`, and the Rust bridge is replaced by an in-process
Kestrel WebSocket server. Run the suite with `dotnet test` from `teams-bot/`. The ASP.NET Core 8
runtime must be installed.
