// Starts the WebSocket bridge client with the host and, on shutdown, leaves any meeting and
// gives queued frames a chance to reach the Rust app before the process exits.

using TeamsBot.Core.Bridge;
using TeamsBot.Core.Hosting;
using TeamsBot.Core.Meeting;

namespace TeamsBot;

/// <summary>Owns the start and orderly stop of the bridge and the meeting.</summary>
internal sealed partial class BotLifetimeService(
    WebSocketClient bridge,
    BotService botService,
    TeamsBotOptions botOptions,
    ILogger<BotLifetimeService> logger) : IHostedService
{
    /// <summary>Starts connecting to the Rust app (retries in the background until it is up).</summary>
    /// <param name="cancellationToken">Unused; starting does not block.</param>
    /// <returns>A completed task.</returns>
    /// <remarks>Side effects: Starts the client's connection loop; logs the target (not the secret).</remarks>
    public Task StartAsync(CancellationToken cancellationToken)
    {
        if (string.IsNullOrEmpty(botOptions.SharedSecret))
        {
            LogNoSharedSecret();
        }
        bridge.Start();
        LogBridgeStarted(botOptions.WebSocketUrl);
        return Task.CompletedTask;
    }

    /// <summary>Leaves the meeting, flushes, and closes the bridge.</summary>
    /// <param name="cancellationToken">Signals that shutdown must finish now.</param>
    /// <returns>A task that completes when everything stopped.</returns>
    /// <remarks>Side effects: Hangs up; clears the recording status; closes the WebSocket.</remarks>
    public async Task StopAsync(CancellationToken cancellationToken)
    {
        await botService.DisposeAsync().ConfigureAwait(false);
        await bridge.FlushAsync(TimeSpan.FromSeconds(5)).ConfigureAwait(false);
        await bridge.DisposeAsync().ConfigureAwait(false);
    }

    [LoggerMessage(Level = LogLevel.Warning, Message = "TeamsBot:SharedSecret is not set; the Rust app accepts the bridge only if it has no secret configured either")]
    private partial void LogNoSharedSecret();

    [LoggerMessage(Level = LogLevel.Information, Message = "Streaming to the Rust app at {WebSocketUrl}")]
    private partial void LogBridgeStarted(Uri webSocketUrl);
}
