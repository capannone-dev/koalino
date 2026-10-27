// End-to-end test of the bot's portable half: a mocked Teams call (NSubstitute) drives the real
// BotService, AudioProcessor, VideoProcessor, and WebSocketClient against an in-process mock of
// the Rust bridge server, which decodes what arrives with the shared Protobuf schema.

using NSubstitute;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Audio;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Meeting;
using TeamsBot.Core.Model;
using TeamsBot.Core.Tests.Support;
using TeamsBot.Core.Video;
using NoteTaker.TeamsBridge.V1;
using Xunit;

namespace TeamsBot.Core.Tests.Integration;

/// <summary>BotService → WebSocketClient → mock Rust server.</summary>
public sealed class BotToBridgeIntegrationTests
{
    private static readonly Uri JoinUrl = new("https://teams.microsoft.com/l/meetup-join/19%3ameeting%40thread.v2/0");

    /// <summary>A meeting's metadata, tagged audio, and screen-share slide reach the server in order, and the end flushes everything.</summary>
    [Fact]
    public async Task AMeetingStreamsMetadataAudioAndSlidesToTheRustApp()
    {
        await using var server = await MockBridgeServer.StartAsync(expectedSecret: "integration-secret");
        var ada = new ParticipantInfo("aad-ada", "Ada Lovelace", "ada@example.com");
        var session = Substitute.For<ICallSession>();
        session.MeetingId.Returns("19:meeting@thread.v2");
        session.Title.Returns("Design review");
        session.StartTime.Returns(DateTimeOffset.FromUnixTimeMilliseconds(1_793_000_000_000));
        session.Participants.Returns(new[] { new RosterParticipant(ada, new uint[] { 11 }) });
        var joiner = Substitute.For<ICallJoiner>();
        joiner.JoinAsync(JoinUrl, Arg.Any<CancellationToken>()).Returns(session);

        await using var bridge = new WebSocketClient(
            new WebSocketClientOptions { Url = server.Url, SharedSecret = "integration-secret" },
            new RecordingLogger<WebSocketClient>());
        bridge.Start();
        using var loggers = new RecordingLoggerFactory();
        await using var bot = new BotService(
            joiner,
            bridge,
            new BotServiceOptions { IdleFlushInterval = null },
            new AudioChunkingOptions(),
            new VideoProcessingOptions(),
            loggers);

        Assert.True(await bot.JoinMeetingAsync(JoinUrl, CancellationToken.None));
        for (ulong offsetMs = 1_000; offsetMs < 1_500; offsetMs += 20)
        {
            session.AudioFrameReceived += Raise.Event<EventHandler<UnmixedAudioFrame>>(
                session,
                new UnmixedAudioFrame(11, Enumerable.Repeat((byte)3, 640).ToArray(), new AudioFormatInfo(16_000, 1), offsetMs));
        }
        session.ScreenShareStateChanged += Raise.Event<EventHandler<bool>>(session, true);
        session.ScreenShareFrameReceived += Raise.Event<EventHandler<RawVideoFrame>>(
            session,
            new RawVideoFrame(Enumerable.Repeat((byte)128, 64 * 32 * 3 / 2).ToArray(), 64, 32, 64, 2_000));
        session.Ended += Raise.Event<EventHandler<CallEnded>>(session, new CallEnded("meeting ended", Unexpected: true));
        Assert.True(await bot.WaitForMeetingEndAsync(TimeSpan.FromSeconds(10)));

        var messages = await server.WaitForBinaryFramesAsync(3, TimeSpan.FromSeconds(10));

        Assert.Equal(TeamsBridgeMessage.PayloadOneofCase.Metadata, messages[0].PayloadCase);
        Assert.Equal("19:meeting@thread.v2", messages[0].Metadata.MeetingId);
        Assert.Equal(1_793_000_000_000u, messages[0].Metadata.StartTimeMs);
        Assert.Equal("aad-ada", Assert.Single(messages[0].Metadata.Participants).ExternalId);

        var video = Assert.Single(messages, message => message.PayloadCase == TeamsBridgeMessage.PayloadOneofCase.Video).Video;
        Assert.Equal("screen_share", video.Source);
        Assert.Equal(2_000u, video.TimestampMs);
        Assert.Equal(new byte[] { 0xFF, 0xD8, 0xFF }, video.ImageData.Span[..3].ToArray());

        var audio = Assert.Single(messages, message => message.PayloadCase == TeamsBridgeMessage.PayloadOneofCase.Audio).Audio;
        Assert.Equal("aad-ada", audio.Participant.ExternalId);
        Assert.Equal("Ada Lovelace", audio.Participant.DisplayName);
        Assert.Equal(16_000u, audio.SampleRate);
        Assert.Equal(1_000u, audio.TimestampMs);
        Assert.Equal(500 * PcmAudioChunk.BytesPerMillisecond, audio.AudioData.Length);
    }
}
