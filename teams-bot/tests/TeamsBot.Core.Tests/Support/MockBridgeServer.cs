// Test helper: an in-process Kestrel WebSocket server that behaves like the Rust Teams bridge
// (Issue #33): optional {"auth":...} handshake answered with {"auth":"ok"} or close 1008, then
// binary TeamsBridgeMessage frames. Tests can refuse connections, drop them, and inspect frames.

using System.Collections.Concurrent;
using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using Microsoft.AspNetCore.Builder;
using Microsoft.AspNetCore.Hosting;
using Microsoft.AspNetCore.Hosting.Server;
using Microsoft.AspNetCore.Hosting.Server.Features;
using Microsoft.AspNetCore.Http;
using Microsoft.AspNetCore.Http.Features;
using Microsoft.Extensions.DependencyInjection;
using Microsoft.Extensions.Logging;
using NoteTaker.TeamsBridge.V1;

namespace TeamsBot.Core.Tests.Support;

/// <summary>A scriptable stand-in for the Rust WebSocket server.</summary>
public sealed class MockBridgeServer : IAsyncDisposable
{
    private readonly WebApplication application;
    private readonly string? expectedSecret;
    private readonly ConcurrentQueue<ReceivedFrame> frames = new();
    private readonly ConcurrentQueue<bool> originHeaders = new();
    private readonly SemaphoreSlim frameArrived = new(0);
    private int connectionCount;

    private MockBridgeServer(WebApplication application, string? expectedSecret)
    {
        this.application = application;
        this.expectedSecret = expectedSecret;
    }

    /// <summary>The <c>ws://</c> URL of the server.</summary>
    public Uri Url { get; private set; } = new("ws://127.0.0.1:1/");

    /// <summary>When true, HTTP requests are answered with 503 instead of a WebSocket.</summary>
    public bool RefuseConnections { get; set; }

    /// <summary>When set, the server aborts a connection after receiving this many binary frames on it.</summary>
    public int? DropAfterBinaryFrames { get; set; }

    /// <summary>How many WebSocket connections were accepted.</summary>
    public int ConnectionCount => connectionCount;

    /// <summary>Whether any handshake carried an <c>Origin</c> header.</summary>
    public bool SawOriginHeader => originHeaders.Any(present => present);

    /// <summary>Every frame received, in order.</summary>
    public IReadOnlyList<ReceivedFrame> Frames => frames.ToArray();

    /// <summary>Starts a server on an ephemeral loopback port.</summary>
    /// <param name="expectedSecret">The secret the client must send first, or <c>null</c>.</param>
    /// <returns>The running server.</returns>
    public static async Task<MockBridgeServer> StartAsync(string? expectedSecret = null)
    {
        var builder = WebApplication.CreateSlimBuilder();
        builder.Logging.ClearProviders();
        builder.WebHost.UseUrls("http://127.0.0.1:0");
        var application = builder.Build();
        var server = new MockBridgeServer(application, expectedSecret);
        application.UseWebSockets();
        application.Run(server.HandleAsync);
        await application.StartAsync();
        var address = application.Services.GetRequiredService<IServer>().Features.GetRequiredFeature<IServerAddressesFeature>().Addresses.First();
        server.Url = new Uri(address.Replace("http://", "ws://", StringComparison.Ordinal) + "/");
        return server;
    }

    /// <summary>Waits until at least <paramref name="count"/> binary frames have arrived.</summary>
    /// <param name="count">The number of binary frames.</param>
    /// <param name="timeout">How long to wait.</param>
    /// <returns>The binary frames, decoded.</returns>
    public async Task<IReadOnlyList<TeamsBridgeMessage>> WaitForBinaryFramesAsync(int count, TimeSpan timeout)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (BinaryMessages().Count < count)
        {
            var remaining = deadline - DateTime.UtcNow;
            if (remaining <= TimeSpan.Zero || !await frameArrived.WaitAsync(remaining))
            {
                throw new TimeoutException($"Expected {count} binary frames, received {BinaryMessages().Count}.");
            }
        }
        return BinaryMessages();
    }

    /// <summary>All binary frames decoded as bridge messages.</summary>
    /// <returns>The messages in arrival order.</returns>
    public IReadOnlyList<TeamsBridgeMessage> BinaryMessages() =>
        frames.Where(frame => frame.IsBinary).Select(frame => TeamsBridgeMessage.Parser.ParseFrom(frame.Payload)).ToList();

    /// <inheritdoc />
    public async ValueTask DisposeAsync()
    {
        await application.StopAsync();
        await application.DisposeAsync();
        frameArrived.Dispose();
    }

    /// <summary>Handles one HTTP request.</summary>
    private async Task HandleAsync(HttpContext context)
    {
        originHeaders.Enqueue(context.Request.Headers.ContainsKey("Origin"));
        if (RefuseConnections || !context.WebSockets.IsWebSocketRequest)
        {
            context.Response.StatusCode = StatusCodes.Status503ServiceUnavailable;
            return;
        }
        using var socket = await context.WebSockets.AcceptWebSocketAsync();
        Interlocked.Increment(ref connectionCount);
        var buffer = new byte[16 * 1024 * 1024];
        var authenticated = expectedSecret is null;
        var binaryFramesOnConnection = 0;
        try
        {
            while (socket.State == WebSocketState.Open)
            {
                var (messageType, payload) = await ReceiveAsync(socket, buffer);
                if (messageType == WebSocketMessageType.Close)
                {
                    await socket.CloseOutputAsync(WebSocketCloseStatus.NormalClosure, null, CancellationToken.None);
                    return;
                }
                frames.Enqueue(new ReceivedFrame(messageType == WebSocketMessageType.Binary, payload));
                frameArrived.Release();
                if (!authenticated)
                {
                    if (messageType == WebSocketMessageType.Text && IsCorrectSecret(payload))
                    {
                        authenticated = true;
                        await socket.SendAsync(Encoding.UTF8.GetBytes("{\"auth\":\"ok\"}"), WebSocketMessageType.Text, true, CancellationToken.None);
                        continue;
                    }
                    await socket.CloseAsync(WebSocketCloseStatus.PolicyViolation, "authentication required", CancellationToken.None);
                    return;
                }
                if (messageType == WebSocketMessageType.Binary && ++binaryFramesOnConnection == DropAfterBinaryFrames)
                {
                    DropAfterBinaryFrames = null;
                    socket.Abort();
                    return;
                }
            }
        }
        catch (WebSocketException)
        {
            // The client went away; the next connection is handled by a new request.
        }
    }

    /// <summary>Checks the {"auth":"..."} frame.</summary>
    private bool IsCorrectSecret(byte[] payload)
    {
        try
        {
            using var document = JsonDocument.Parse(payload);
            return document.RootElement.GetProperty("auth").GetString() == expectedSecret;
        }
        catch (Exception exception) when (exception is JsonException or KeyNotFoundException or InvalidOperationException)
        {
            return false;
        }
    }

    /// <summary>Receives one complete message.</summary>
    private static async Task<(WebSocketMessageType Type, byte[] Payload)> ReceiveAsync(WebSocket socket, byte[] buffer)
    {
        var length = 0;
        while (true)
        {
            var result = await socket.ReceiveAsync(buffer.AsMemory(length), CancellationToken.None);
            length += result.Count;
            if (result.EndOfMessage)
            {
                return (result.MessageType, buffer.AsSpan(0, length).ToArray());
            }
        }
    }

    /// <summary>One received WebSocket message.</summary>
    /// <param name="IsBinary">Binary (protocol) or text (auth) frame.</param>
    /// <param name="Payload">The message bytes.</param>
    public sealed record ReceivedFrame(bool IsBinary, byte[] Payload)
    {
        /// <summary>The payload as UTF-8 text.</summary>
        public string Text => Encoding.UTF8.GetString(Payload);
    }
}
