// HTTP endpoints of the bot:
//   POST /api/calling  Graph calling webhook (the SDK validates the caller's JWT).
//   POST /api/join     {"joinUrl": "..."} with X-Api-Key; the bot joins in the background (202).
//   POST /api/leave    with X-Api-Key; leaves the current meeting.
//   GET  /healthz      bridge connection and meeting state (no identifiers).

using Microsoft.AspNetCore.Http.Extensions;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Hosting;
using TeamsBot.Core.Meeting;
using TeamsBot.Media;

namespace TeamsBot;

/// <summary>Maps the bot's HTTP endpoints.</summary>
internal static partial class BotEndpoints
{
    /// <summary>The header carrying the control API key.</summary>
    private const string ApiKeyHeader = "X-Api-Key";

    /// <summary>Request body of <c>POST /api/join</c>.</summary>
    /// <param name="JoinUrl">The meeting's Teams join link.</param>
    internal sealed record JoinRequest(Uri? JoinUrl);

    /// <summary>Registers the endpoints.</summary>
    /// <param name="app">The application.</param>
    /// <returns>The application, for chaining.</returns>
    public static WebApplication MapBotEndpoints(this WebApplication app)
    {
        app.MapPost("/api/calling", HandleCallingNotificationAsync);
        app.MapPost("/api/join", HandleJoin);
        app.MapPost("/api/leave", HandleLeaveAsync);
        app.MapGet("/healthz", (WebSocketClient bridge, BotService bot) =>
            Results.Ok(new { bridgeConnected = bridge.IsConnected, inMeeting = bot.IsInMeeting, bufferedAudioMs = bridge.BufferedAudioMs }));
        return app;
    }

    /// <summary>Relays a Graph callback into the SDK and copies its response back.</summary>
    /// <remarks>Side effects: Updates call state; may raise meeting events.</remarks>
    private static async Task HandleCallingNotificationAsync(HttpContext context, GraphCallJoiner joiner)
    {
        using var request = await ToHttpRequestMessageAsync(context.Request).ConfigureAwait(false);
        using var response = await joiner.ProcessNotificationAsync(request).ConfigureAwait(false);
        context.Response.StatusCode = (int)response.StatusCode;
        foreach (var header in response.Headers.Concat(response.Content.Headers))
        {
            if (!string.Equals(header.Key, "Transfer-Encoding", StringComparison.OrdinalIgnoreCase))
            {
                context.Response.Headers[header.Key] = header.Value.ToArray();
            }
        }
        await response.Content.CopyToAsync(context.Response.Body, context.RequestAborted).ConfigureAwait(false);
    }

    /// <summary>Validates the request and starts joining in the background.</summary>
    /// <remarks>Side effects: Starts a join; the outcome is logged.</remarks>
    private static IResult HandleJoin(
        HttpRequest request,
        JoinRequest body,
        ApiKeyVerifier apiKeys,
        BotService bot,
        IHostApplicationLifetime lifetime,
        ILoggerFactory loggerFactory)
    {
        if (!apiKeys.IsEnabled)
        {
            return Results.Problem("The join endpoint is disabled: set TeamsBot:ApiKey (at least 32 characters).", statusCode: StatusCodes.Status503ServiceUnavailable);
        }
        if (!apiKeys.IsAuthorized(request.Headers[ApiKeyHeader]))
        {
            return Results.Unauthorized();
        }
        if (body.JoinUrl is null)
        {
            return Results.Problem("joinUrl is required.", statusCode: StatusCodes.Status400BadRequest);
        }
        if (!TeamsJoinUrl.TryParse(body.JoinUrl, out _, out var error))
        {
            return Results.Problem(error, statusCode: StatusCodes.Status400BadRequest);
        }
        if (bot.IsInMeeting)
        {
            return Results.Conflict(new { error = "The bot is already in a meeting." });
        }

        var logger = loggerFactory.CreateLogger(typeof(BotEndpoints));
        _ = Task.Run(async () =>
        {
            try
            {
                if (!await bot.JoinMeetingAsync(body.JoinUrl, lifetime.ApplicationStopping).ConfigureAwait(false))
                {
                    LogJoinNotStarted(logger);
                }
            }
            catch (OperationCanceledException)
            {
                LogJoinCancelled(logger);
            }
        });
        return Results.Accepted("/healthz", new { status = "joining" });
    }

    /// <summary>Leaves the current meeting.</summary>
    /// <remarks>Side effects: Hangs up; flushes media.</remarks>
    private static async Task<IResult> HandleLeaveAsync(HttpRequest request, ApiKeyVerifier apiKeys, BotService bot, CancellationToken cancellationToken)
    {
        if (!apiKeys.IsAuthorized(request.Headers[ApiKeyHeader]))
        {
            return Results.Unauthorized();
        }
        await bot.LeaveAsync(cancellationToken).ConfigureAwait(false);
        return Results.NoContent();
    }

    /// <summary>Converts an ASP.NET Core request into the HttpRequestMessage the Graph SDK expects.</summary>
    private static async Task<HttpRequestMessage> ToHttpRequestMessageAsync(HttpRequest request)
    {
        var body = new MemoryStream();
        await request.Body.CopyToAsync(body, request.HttpContext.RequestAborted).ConfigureAwait(false);
        body.Position = 0;
        var message = new HttpRequestMessage(new HttpMethod(request.Method), new Uri(request.GetEncodedUrl()))
        {
            Content = new StreamContent(body),
        };
        foreach (var header in request.Headers)
        {
            if (!message.Headers.TryAddWithoutValidation(header.Key, header.Value.ToArray()))
            {
                message.Content.Headers.TryAddWithoutValidation(header.Key, header.Value.ToArray());
            }
        }
        return message;
    }

    [LoggerMessage(Level = LogLevel.Warning, Message = "The join request did not start streaming; see the preceding errors")]
    private static partial void LogJoinNotStarted(ILogger logger);

    [LoggerMessage(Level = LogLevel.Information, Message = "The join was cancelled because the host is stopping")]
    private static partial void LogJoinCancelled(ILogger logger);
}
