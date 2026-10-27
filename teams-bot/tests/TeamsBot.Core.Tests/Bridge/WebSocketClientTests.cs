// Specifies the bridge WebSocket client against an in-process mock of the Rust server:
// authentication, metadata ordering, reconnection with backoff, and drop-oldest buffering.

using Microsoft.Extensions.Logging;
using NoteTaker.TeamsBridge.V1;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Model;
using TeamsBot.Core.Tests.Support;
using Xunit;

namespace TeamsBot.Core.Tests.Bridge;

/// <summary>Tests for <see cref="WebSocketClient"/>.</summary>
public sealed class WebSocketClientTests
{
    private static readonly TimeSpan Wait = TimeSpan.FromSeconds(10);
    private static readonly ParticipantInfo Ada = new("aad-ada", "Ada Lovelace");
    private static readonly MeetingInfo Meeting = new("19:meeting@thread.v2", "Sync", DateTimeOffset.UnixEpoch, new[] { Ada });

    private readonly RecordingLogger<WebSocketClient> logger = new();
    private readonly List<TimeSpan> recordedDelays = new();

    /// <summary>Creates a client whose backoff delays are recorded and shortened to 20 ms.</summary>
    private WebSocketClient Client(Uri url, string? secret = null, int bufferedAudioMs = 30_000) =>
        new(
            new WebSocketClientOptions { Url = url, SharedSecret = secret, MaximumBufferedAudioMs = bufferedAudioMs, Jitter = 0 },
            logger,
            async (delay, cancellationToken) =>
            {
                lock (recordedDelays)
                {
                    recordedDelays.Add(delay);
                }
                await Task.Delay(TimeSpan.FromMilliseconds(20), cancellationToken);
            });

    /// <summary>One second of audio stamped <paramref name="timestampMs"/>.</summary>
    private static PcmAudioChunk Audio(ulong timestampMs) => new(Ada, new byte[32_000], timestampMs);

    /// <summary>With a secret, the first frame is the auth text frame, then metadata, then media.</summary>
    [Fact]
    public async Task AuthenticatesThenSendsMetadataBeforeMedia()
    {
        await using var server = await MockBridgeServer.StartAsync(expectedSecret: "s3cret-token");
        await using var client = Client(server.Url, "s3cret-token");
        client.Start();
        client.SetMeeting(Meeting);
        client.EnqueueAudio(Audio(1_000));
        client.EnqueueVideo(new EncodedVideoFrame(new byte[] { 0xFF, 0xD8, 0xFF }, 2_000));

        var messages = await server.WaitForBinaryFramesAsync(3, Wait);

        var first = server.Frames[0];
        Assert.False(first.IsBinary);
        Assert.Equal("{\"auth\":\"s3cret-token\"}", first.Text);
        Assert.Equal(TeamsBridgeMessage.PayloadOneofCase.Metadata, messages[0].PayloadCase);
        Assert.Equal(1_000u, messages[1].Audio.TimestampMs);
        Assert.Equal("screen_share", messages[2].Video.Source);
        Assert.False(server.SawOriginHeader, "browsers send Origin; the bot must not");
        Assert.False(logger.ContainsAnywhere("s3cret-token"), "the secret must never be logged");
    }

    /// <summary>Without a secret, no auth frame is sent.</summary>
    [Fact]
    public async Task WithoutASecretNoAuthFrameIsSent()
    {
        await using var server = await MockBridgeServer.StartAsync();
        await using var client = Client(server.Url);
        client.Start();
        client.SetMeeting(Meeting);

        await server.WaitForBinaryFramesAsync(1, Wait);

        Assert.All(server.Frames, frame => Assert.True(frame.IsBinary));
    }

    /// <summary>A rejected secret (close 1008) is logged as an error and retried with the longest backoff.</summary>
    [Fact]
    public async Task RejectedSecretBacksOffWithoutCrashing()
    {
        await using var server = await MockBridgeServer.StartAsync(expectedSecret: "right-secret");
        await using var client = Client(server.Url, "wrong-secret");
        client.Start();

        await WaitUntilAsync(() => server.ConnectionCount >= 2);

        Assert.True(logger.Contains(LogLevel.Error, "rejected"));
        lock (recordedDelays)
        {
            Assert.Contains(TimeSpan.FromSeconds(30), recordedDelays);
        }
        Assert.False(logger.ContainsAnywhere("wrong-secret"));
    }

    /// <summary>After a dropped connection the client reconnects, resends metadata first, and delivers the rest.</summary>
    [Fact]
    public async Task ReconnectsAndResendsMetadataFirst()
    {
        await using var server = await MockBridgeServer.StartAsync();
        server.DropAfterBinaryFrames = 2;
        await using var client = Client(server.Url);
        client.Start();
        client.SetMeeting(Meeting);
        client.EnqueueAudio(Audio(0));
        await server.WaitForBinaryFramesAsync(2, Wait);
        await WaitUntilAsync(() => server.ConnectionCount == 2 && client.IsConnected);
        client.EnqueueAudio(Audio(1_000));
        client.EnqueueAudio(Audio(2_000));

        var messages = await server.WaitForBinaryFramesAsync(5, Wait);

        Assert.Equal(2, server.ConnectionCount);
        Assert.Equal(TeamsBridgeMessage.PayloadOneofCase.Metadata, messages[2].PayloadCase);
        var audioTimestamps = messages.Where(message => message.PayloadCase == TeamsBridgeMessage.PayloadOneofCase.Audio)
            .Select(message => message.Audio.TimestampMs);
        Assert.Equal(new ulong[] { 0, 1_000, 2_000 }, audioTimestamps);
        Assert.True(logger.Contains(LogLevel.Warning, "disconnected"));
    }

    /// <summary>While disconnected, at most 30 s of audio is kept; the oldest is dropped with a warning.</summary>
    [Fact]
    public async Task BuffersTheNewestThirtySecondsWhileDisconnected()
    {
        await using var server = await MockBridgeServer.StartAsync();
        server.RefuseConnections = true;
        await using var client = Client(server.Url);
        client.Start();
        client.SetMeeting(Meeting);
        for (ulong second = 0; second < 40; second++)
        {
            client.EnqueueAudio(Audio(second * 1_000));
        }
        Assert.Equal(30_000, client.BufferedAudioMs);
        Assert.True(logger.Contains(LogLevel.Warning, "dropped"));

        server.RefuseConnections = false;
        var messages = await server.WaitForBinaryFramesAsync(31, Wait);

        Assert.Equal(TeamsBridgeMessage.PayloadOneofCase.Metadata, messages[0].PayloadCase);
        Assert.Equal(10_000u, messages[1].Audio.TimestampMs);
        Assert.Equal(39_000u, messages[30].Audio.TimestampMs);
    }

    /// <summary>Queued screen-share frames are capped; the oldest is dropped.</summary>
    [Fact]
    public async Task VideoQueueIsCapped()
    {
        await using var server = await MockBridgeServer.StartAsync();
        server.RefuseConnections = true;
        await using var client = Client(server.Url);
        client.Start();
        client.SetMeeting(Meeting);
        for (ulong frame = 0; frame < WebSocketClientOptions.DefaultMaximumBufferedVideoFrames + 4; frame++)
        {
            client.EnqueueVideo(new EncodedVideoFrame(new byte[] { 0xFF, 0xD8, 0xFF }, frame));
        }

        server.RefuseConnections = false;
        var messages = await server.WaitForBinaryFramesAsync(WebSocketClientOptions.DefaultMaximumBufferedVideoFrames + 1, Wait);

        Assert.Equal(4u, messages[1].Video.TimestampMs);
    }

    /// <summary>Flushing waits until queued frames are sent.</summary>
    [Fact]
    public async Task FlushWaitsForTheQueueToDrain()
    {
        await using var server = await MockBridgeServer.StartAsync();
        await using var client = Client(server.Url);
        client.Start();
        client.SetMeeting(Meeting);
        for (ulong second = 0; second < 5; second++)
        {
            client.EnqueueAudio(Audio(second * 1_000));
        }

        Assert.True(await client.FlushAsync(Wait));

        Assert.Equal(0, client.BufferedAudioMs);
        await server.WaitForBinaryFramesAsync(6, Wait);
    }

    /// <summary>Invalid media is rejected at enqueue time (it would be skipped by the Rust app).</summary>
    [Fact]
    public async Task InvalidMediaIsRejectedAtEnqueue()
    {
        await using var client = Client(new Uri("ws://127.0.0.1:9/"));

        Assert.False(client.EnqueueAudio(new PcmAudioChunk(Ada, new byte[3], 0)));
        Assert.True(logger.Contains(LogLevel.Warning, "rejected"));
    }

    /// <summary>Disposing a client that never connected does not throw.</summary>
    [Fact]
    public async Task DisposingAnUnconnectedClientIsSafe()
    {
        var client = Client(new Uri("ws://127.0.0.1:9/"));
        client.Start();

        await client.DisposeAsync();
    }

    /// <summary>Backoff doubles from 1 s to a 30 s cap, with bounded jitter.</summary>
    [Fact]
    public void BackoffDoublesUpToTheCap()
    {
        var delays = Enumerable.Range(0, 8)
            .Select(attempt => WebSocketClient.ComputeBackoff(attempt, TimeSpan.FromSeconds(1), TimeSpan.FromSeconds(30), jitter: 0, random: 0.5))
            .Select(delay => delay.TotalSeconds);
        Assert.Equal(new double[] { 1, 2, 4, 8, 16, 30, 30, 30 }, delays);

        var jittered = WebSocketClient.ComputeBackoff(3, TimeSpan.FromSeconds(1), TimeSpan.FromSeconds(30), jitter: 0.2, random: 1.0);
        Assert.Equal(9.6, jittered.TotalSeconds, precision: 3);
    }

    /// <summary>Polls a condition.</summary>
    private static async Task WaitUntilAsync(Func<bool> condition)
    {
        var deadline = DateTime.UtcNow + Wait;
        while (!condition())
        {
            Assert.True(DateTime.UtcNow < deadline, "condition not met in time");
            await Task.Delay(20);
        }
    }
}
