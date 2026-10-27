// The bot's connection to the Rust app's Teams bridge (ws://localhost:8765 by default, Issue #33).
//
// One background loop connects, authenticates with the optional shared secret
// ({"auth":"<secret>"} answered by {"auth":"ok"}; a 1008 close means the secret was rejected), and
// then sends queued frames in order. Media is enqueued without blocking the media threads. While
// disconnected, frames wait in a bounded queue: at most MaximumBufferedAudioMs of audio (default
// 30 s) and MaximumBufferedVideoFrames slides; when full, the oldest are dropped with a
// rate-limited warning. Reconnects use exponential backoff with jitter, and the meeting metadata
// is sent again first after every reconnect. The protocol has no acknowledgements, so a frame
// written just before an undetected drop can be lost. The secret and media are never logged.

using System.Net.WebSockets;
using System.Text;
using System.Text.Json;
using Microsoft.Extensions.Logging;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Model;

namespace TeamsBot.Core.Bridge;

/// <summary>Connection settings (configuration <c>TeamsBot:WebSocketUrl</c>, <c>TeamsBot:SharedSecret</c>).</summary>
public sealed class WebSocketClientOptions
{
    /// <summary>Default cap on queued screen-share frames.</summary>
    public const int DefaultMaximumBufferedVideoFrames = 16;

    /// <summary>The Rust app's bridge endpoint (default <c>ws://localhost:8765</c>).</summary>
    public Uri Url { get; set; } = new("ws://localhost:8765");

    /// <summary>The shared secret (<c>NOTE_TAKER_TEAMS_BRIDGE_SECRET</c> on the Rust side); <c>null</c> when unset. Never logged.</summary>
    public string? SharedSecret { get; set; }

    /// <summary>First reconnect delay (default 1 s).</summary>
    public TimeSpan InitialBackoff { get; set; } = TimeSpan.FromSeconds(1);

    /// <summary>Longest reconnect delay, also used after a rejected secret (default 30 s).</summary>
    public TimeSpan MaximumBackoff { get; set; } = TimeSpan.FromSeconds(30);

    /// <summary>Random spread applied to each delay, as a fraction (default 0.2 = ±20%).</summary>
    public double Jitter { get; set; } = 0.2;

    /// <summary>Time allowed for the TCP/WebSocket handshake (default 10 s).</summary>
    public TimeSpan ConnectTimeout { get; set; } = TimeSpan.FromSeconds(10);

    /// <summary>Time allowed for the auth acknowledgement (default 5 s, the Rust server's limit).</summary>
    public TimeSpan AuthenticationTimeout { get; set; } = TimeSpan.FromSeconds(5);

    /// <summary>Most audio kept while disconnected, in milliseconds (default 30000).</summary>
    public int MaximumBufferedAudioMs { get; set; } = 30_000;

    /// <summary>Most screen-share frames kept while disconnected (default 16).</summary>
    public int MaximumBufferedVideoFrames { get; set; } = DefaultMaximumBufferedVideoFrames;

    /// <summary>Rejects unusable settings.</summary>
    /// <exception cref="ArgumentException">A value is invalid.</exception>
    public void Validate()
    {
        if (Url is null || (Url.Scheme != "ws" && Url.Scheme != "wss"))
        {
            throw new ArgumentException("WebSocketUrl must be a ws:// or wss:// URL.", nameof(Url));
        }
        if (InitialBackoff <= TimeSpan.Zero || MaximumBackoff < InitialBackoff)
        {
            throw new ArgumentException("Backoff delays must be positive and ordered.", nameof(InitialBackoff));
        }
        if (Jitter is < 0 or >= 1)
        {
            throw new ArgumentException("Jitter must be in [0, 1).", nameof(Jitter));
        }
        if (MaximumBufferedAudioMs < 1 || MaximumBufferedVideoFrames < 1)
        {
            throw new ArgumentException("Buffer limits must be positive.", nameof(MaximumBufferedAudioMs));
        }
    }
}

/// <summary>Streams bridge frames to the Rust app with authentication, reconnection, and buffering.</summary>
public sealed partial class WebSocketClient : IBridgeSink, IAsyncDisposable
{
    /// <summary>Exponent cap so backoff arithmetic never overflows.</summary>
    private const int MaximumBackoffExponent = 30;

    /// <summary>Minimum spacing between buffer-overflow warnings.</summary>
    private static readonly TimeSpan OverflowWarningInterval = TimeSpan.FromSeconds(5);

    private readonly WebSocketClientOptions options;
    private readonly ILogger<WebSocketClient> logger;
    private readonly Func<TimeSpan, CancellationToken, Task> delayAsync;
    private readonly LinkedList<OutboundFrame> queue = new();
    private readonly object gate = new();
    private readonly SemaphoreSlim frameAvailable = new(0);
    private readonly CancellationTokenSource stopping = new();
    private Task? connectionLoop;
    private OutboundFrame? inFlight;
    private byte[]? lastSentMetadata;
    private int bufferedAudioMs;
    private int bufferedVideoFrames;
    private int unreportedDroppedFrames;
    private DateTime lastOverflowWarningUtc = DateTime.MinValue;
    private volatile bool connected;

    /// <summary>Creates a client; call <see cref="Start"/> to connect.</summary>
    /// <param name="options">Connection settings; validated here.</param>
    /// <param name="logger">Receives connection status and warnings (never secrets or media).</param>
    /// <param name="delayAsync">Waits between reconnects; defaults to <see cref="Task.Delay(TimeSpan, CancellationToken)"/> (tests shorten it).</param>
    /// <exception cref="ArgumentException">The options are invalid.</exception>
    public WebSocketClient(
        WebSocketClientOptions options,
        ILogger<WebSocketClient> logger,
        Func<TimeSpan, CancellationToken, Task>? delayAsync = null)
    {
        ArgumentNullException.ThrowIfNull(options);
        options.Validate();
        this.options = options;
        this.logger = logger ?? throw new ArgumentNullException(nameof(logger));
        this.delayAsync = delayAsync ?? Task.Delay;
    }

    /// <summary>Whether a connection is currently authenticated and open.</summary>
    public bool IsConnected => connected;

    /// <summary>Audio waiting to be sent, in milliseconds.</summary>
    public int BufferedAudioMs
    {
        get
        {
            lock (gate)
            {
                return bufferedAudioMs;
            }
        }
    }

    /// <summary>Starts the background connection loop (idempotent).</summary>
    public void Start()
    {
        lock (gate)
        {
            connectionLoop ??= Task.Run(() => RunAsync(stopping.Token));
        }
    }

    /// <summary>Queues meeting metadata; it is also re-sent first after every reconnect.</summary>
    /// <param name="meeting">The meeting being streamed.</param>
    /// <returns><c>false</c> when the metadata is invalid (logged).</returns>
    public bool SetMeeting(MeetingInfo meeting) => Enqueue(FrameKind.Metadata, () => ProtobufSerializer.SerializeMetadata(meeting), 0);

    /// <summary>Queues one audio chunk without blocking.</summary>
    /// <param name="chunk">The chunk.</param>
    /// <returns><c>false</c> when the chunk is invalid (logged).</returns>
    public bool EnqueueAudio(PcmAudioChunk chunk)
    {
        ArgumentNullException.ThrowIfNull(chunk);
        return Enqueue(FrameKind.Audio, () => ProtobufSerializer.SerializeAudio(chunk), (int)chunk.DurationMs);
    }

    /// <summary>Queues one screen-share frame without blocking.</summary>
    /// <param name="frame">The JPEG frame.</param>
    /// <returns><c>false</c> when the frame is invalid (logged).</returns>
    public bool EnqueueVideo(EncodedVideoFrame frame) => Enqueue(FrameKind.Video, () => ProtobufSerializer.SerializeVideo(frame), 0);

    /// <summary>Waits until every queued frame has been sent.</summary>
    /// <param name="timeout">How long to wait (e.g. while a meeting ends).</param>
    /// <returns><c>true</c> when the queue drained in time.</returns>
    public async Task<bool> FlushAsync(TimeSpan timeout)
    {
        var deadline = DateTime.UtcNow + timeout;
        while (true)
        {
            lock (gate)
            {
                if (queue.Count == 0 && inFlight is null)
                {
                    return true;
                }
            }
            if (DateTime.UtcNow >= deadline)
            {
                return false;
            }
            await Task.Delay(TimeSpan.FromMilliseconds(20)).ConfigureAwait(false);
        }
    }

    /// <summary>Computes a reconnect delay.</summary>
    /// <param name="attempt">Failed attempts so far (0 for the first retry).</param>
    /// <param name="initial">The first delay.</param>
    /// <param name="maximum">The cap before jitter.</param>
    /// <param name="jitter">Spread as a fraction of the delay.</param>
    /// <param name="random">A value in [0, 1] choosing the point within the spread.</param>
    /// <returns><c>min(maximum, initial·2^attempt) · (1 + jitter·(2·random − 1))</c>.</returns>
    public static TimeSpan ComputeBackoff(int attempt, TimeSpan initial, TimeSpan maximum, double jitter, double random)
    {
        var exponent = Math.Clamp(attempt, 0, MaximumBackoffExponent);
        var baseMs = Math.Min(maximum.TotalMilliseconds, initial.TotalMilliseconds * Math.Pow(2, exponent));
        return TimeSpan.FromMilliseconds(baseMs * (1 + (jitter * ((2 * random) - 1))));
    }

    /// <summary>Stops the loop, closing the connection normally.</summary>
    /// <returns>A task that completes when the loop has ended.</returns>
    public async ValueTask DisposeAsync()
    {
        await stopping.CancelAsync().ConfigureAwait(false);
        Task? loop;
        lock (gate)
        {
            loop = connectionLoop;
        }
        if (loop is not null)
        {
            await loop.ConfigureAwait(false);
        }
        stopping.Dispose();
        frameAvailable.Dispose();
    }

    /// <summary>Serializes and queues a frame, applying the buffer limits.</summary>
    private bool Enqueue(FrameKind kind, Func<byte[]> serialize, int audioMs)
    {
        byte[] payload;
        try
        {
            payload = serialize();
        }
        catch (ArgumentException exception)
        {
            LogFrameRejected(kind, exception.Message);
            return false;
        }
        lock (gate)
        {
            queue.AddLast(new OutboundFrame(kind, payload, audioMs));
            bufferedAudioMs += audioMs;
            bufferedVideoFrames += kind == FrameKind.Video ? 1 : 0;
            EnforceLimits();
        }
        frameAvailable.Release();
        return true;
    }

    /// <summary>Drops the oldest audio and video beyond the limits. Caller holds the lock.</summary>
    private void EnforceLimits()
    {
        var droppedFrames = 0;
        while (bufferedAudioMs > options.MaximumBufferedAudioMs && RemoveOldest(FrameKind.Audio))
        {
            droppedFrames++;
        }
        while (bufferedVideoFrames > options.MaximumBufferedVideoFrames && RemoveOldest(FrameKind.Video))
        {
            droppedFrames++;
        }
        if (droppedFrames == 0)
        {
            return;
        }
        unreportedDroppedFrames += droppedFrames;
        var now = DateTime.UtcNow;
        if (now - lastOverflowWarningUtc >= OverflowWarningInterval)
        {
            LogBufferOverflow(unreportedDroppedFrames, options.MaximumBufferedAudioMs);
            unreportedDroppedFrames = 0;
            lastOverflowWarningUtc = now;
        }
    }

    /// <summary>Removes the oldest queued frame of a kind. Caller holds the lock.</summary>
    private bool RemoveOldest(FrameKind kind)
    {
        for (var node = queue.First; node is not null; node = node.Next)
        {
            if (node.Value.Kind == kind)
            {
                queue.Remove(node);
                Account(node.Value, removed: true);
                return true;
            }
        }
        return false;
    }

    /// <summary>Updates the buffer counters for a frame leaving (or returning to) the queue. Caller holds the lock.</summary>
    private void Account(OutboundFrame frame, bool removed)
    {
        var sign = removed ? -1 : 1;
        bufferedAudioMs += sign * frame.AudioMs;
        bufferedVideoFrames += frame.Kind == FrameKind.Video ? sign : 0;
    }

    /// <summary>Connects, streams, and reconnects until stopped.</summary>
    private async Task RunAsync(CancellationToken cancellationToken)
    {
        var failedAttempts = 0;
        while (!cancellationToken.IsCancellationRequested)
        {
            using var socket = new ClientWebSocket();
            socket.Options.KeepAliveInterval = TimeSpan.FromSeconds(15);
            try
            {
                using (var connectTimeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken))
                {
                    connectTimeout.CancelAfter(options.ConnectTimeout);
                    await socket.ConnectAsync(options.Url, connectTimeout.Token).ConfigureAwait(false);
                }
                if (options.SharedSecret is { } sharedSecret)
                {
                    await AuthenticateAsync(socket, sharedSecret, cancellationToken).ConfigureAwait(false);
                }
                connected = true;
                failedAttempts = 0;
                LogConnected(options.Url);
                await StreamAsync(socket, cancellationToken).ConfigureAwait(false);
                if (!cancellationToken.IsCancellationRequested)
                {
                    LogDisconnected("the server closed the connection");
                }
            }
            catch (AuthenticationRejectedException)
            {
                LogAuthenticationRejected(options.Url);
                failedAttempts = MaximumBackoffExponent;
            }
            catch (OperationCanceledException) when (cancellationToken.IsCancellationRequested)
            {
                break;
            }
            catch (Exception exception) when (exception is WebSocketException or OperationCanceledException or IOException or InvalidOperationException)
            {
                LogDisconnected(exception.Message);
            }
            finally
            {
                connected = false;
            }
            if (cancellationToken.IsCancellationRequested)
            {
                break;
            }
            var delay = ComputeBackoff(failedAttempts++, options.InitialBackoff, options.MaximumBackoff, options.Jitter, Random.Shared.NextDouble());
            try
            {
                await delayAsync(delay, cancellationToken).ConfigureAwait(false);
            }
            catch (OperationCanceledException)
            {
                break;
            }
        }
    }

    /// <summary>Sends <c>{"auth":"…"}</c> and waits for <c>{"auth":"ok"}</c>.</summary>
    /// <exception cref="AuthenticationRejectedException">The server closed with 1008 or answered otherwise.</exception>
    private async Task AuthenticateAsync(ClientWebSocket socket, string sharedSecret, CancellationToken cancellationToken)
    {
        var request = JsonSerializer.SerializeToUtf8Bytes(new AuthenticationRequest(sharedSecret));
        await socket.SendAsync(request, WebSocketMessageType.Text, endOfMessage: true, cancellationToken).ConfigureAwait(false);
        using var timeout = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        timeout.CancelAfter(options.AuthenticationTimeout);
        var buffer = new byte[256];
        var result = await socket.ReceiveAsync(buffer, timeout.Token).ConfigureAwait(false);
        if (result.MessageType == WebSocketMessageType.Close)
        {
            throw new AuthenticationRejectedException();
        }
        var reply = Encoding.UTF8.GetString(buffer, 0, result.Count);
        if (result.MessageType != WebSocketMessageType.Text || !IsAcceptedReply(reply))
        {
            throw new AuthenticationRejectedException();
        }
    }

    /// <summary>Checks for <c>{"auth":"ok"}</c>.</summary>
    private static bool IsAcceptedReply(string reply)
    {
        try
        {
            using var document = JsonDocument.Parse(reply);
            return document.RootElement.TryGetProperty("auth", out var auth) && auth.GetString() == "ok";
        }
        catch (JsonException)
        {
            return false;
        }
    }

    /// <summary>Sends queued frames until the connection closes or the client stops.</summary>
    private async Task StreamAsync(ClientWebSocket socket, CancellationToken cancellationToken)
    {
        using var connection = CancellationTokenSource.CreateLinkedTokenSource(cancellationToken);
        var receiving = DrainIncomingAsync(socket, connection);
        try
        {
            byte[]? metadataToResend;
            lock (gate)
            {
                var queueStartsWithMetadata = queue.First?.Value.Kind == FrameKind.Metadata;
                metadataToResend = queueStartsWithMetadata ? null : lastSentMetadata;
            }
            if (metadataToResend is not null)
            {
                await socket.SendAsync(metadataToResend, WebSocketMessageType.Binary, endOfMessage: true, connection.Token).ConfigureAwait(false);
            }
            while (true)
            {
                OutboundFrame? next;
                lock (gate)
                {
                    next = queue.First?.Value;
                    if (next is not null)
                    {
                        queue.RemoveFirst();
                        Account(next, removed: true);
                        inFlight = next;
                    }
                }
                if (next is null)
                {
                    await frameAvailable.WaitAsync(connection.Token).ConfigureAwait(false);
                    continue;
                }
                try
                {
                    await socket.SendAsync(next.Payload, WebSocketMessageType.Binary, endOfMessage: true, connection.Token).ConfigureAwait(false);
                    lock (gate)
                    {
                        inFlight = null;
                        if (next.Kind == FrameKind.Metadata)
                        {
                            lastSentMetadata = next.Payload;
                        }
                    }
                }
                catch
                {
                    lock (gate)
                    {
                        inFlight = null;
                        queue.AddFirst(next);
                        Account(next, removed: false);
                        EnforceLimits();
                    }
                    throw;
                }
            }
        }
        catch (OperationCanceledException) when (!cancellationToken.IsCancellationRequested)
        {
            // The receive side saw the connection close; reconnect.
        }
        finally
        {
            if (cancellationToken.IsCancellationRequested && socket.State == WebSocketState.Open)
            {
                await CloseQuietlyAsync(socket).ConfigureAwait(false);
            }
            await connection.CancelAsync().ConfigureAwait(false);
            await receiving.ConfigureAwait(false);
        }
    }

    /// <summary>Reads (and discards) server frames so close and ping frames are handled; cancels <paramref name="connection"/> on close.</summary>
    private static async Task DrainIncomingAsync(ClientWebSocket socket, CancellationTokenSource connection)
    {
        var buffer = new byte[1024];
        try
        {
            while (socket.State == WebSocketState.Open)
            {
                var result = await socket.ReceiveAsync(buffer, connection.Token).ConfigureAwait(false);
                if (result.MessageType == WebSocketMessageType.Close)
                {
                    break;
                }
            }
        }
        catch (Exception exception) when (exception is WebSocketException or OperationCanceledException or ObjectDisposedException)
        {
            // Connection gone or client stopping.
        }
        finally
        {
            try
            {
                await connection.CancelAsync().ConfigureAwait(false);
            }
            catch (ObjectDisposedException)
            {
                // Already torn down.
            }
        }
    }

    /// <summary>Closes the connection normally, giving up after two seconds.</summary>
    private static async Task CloseQuietlyAsync(ClientWebSocket socket)
    {
        using var timeout = new CancellationTokenSource(TimeSpan.FromSeconds(2));
        try
        {
            await socket.CloseOutputAsync(WebSocketCloseStatus.NormalClosure, "bot shutting down", timeout.Token).ConfigureAwait(false);
        }
        catch (Exception exception) when (exception is WebSocketException or OperationCanceledException)
        {
            // Best effort.
        }
    }

    [LoggerMessage(Level = LogLevel.Information, Message = "Connected to the Rust Teams bridge at {Url}")]
    private partial void LogConnected(Uri url);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Teams bridge disconnected ({Reason}); reconnecting with backoff")]
    private partial void LogDisconnected(string reason);

    [LoggerMessage(Level = LogLevel.Error, Message = "The Rust Teams bridge at {Url} rejected the shared secret; check TeamsBot:SharedSecret and NOTE_TAKER_TEAMS_BRIDGE_SECRET")]
    private partial void LogAuthenticationRejected(Uri url);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Teams bridge buffer full; dropped the oldest {DroppedFrames} frames (audio limit {LimitMs} ms)")]
    private partial void LogBufferOverflow(int droppedFrames, int limitMs);

    [LoggerMessage(Level = LogLevel.Warning, Message = "{Kind} frame rejected before sending: {Reason}")]
    private partial void LogFrameRejected(FrameKind kind, string reason);

    /// <summary>What a queued frame carries.</summary>
    private enum FrameKind
    {
        Metadata,
        Audio,
        Video,
    }

    /// <summary>A serialized frame waiting to be sent.</summary>
    private sealed record OutboundFrame(FrameKind Kind, byte[] Payload, int AudioMs);

    /// <summary>The authentication frame; serialized as <c>{"auth":"…"}</c>.</summary>
    private sealed record AuthenticationRequest([property: System.Text.Json.Serialization.JsonPropertyName("auth")] string Auth);

    /// <summary>The server refused the shared secret.</summary>
    private sealed class AuthenticationRejectedException : Exception
    {
    }
}
