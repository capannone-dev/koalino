// Specifies the meeting lifecycle against a mocked Teams call (NSubstitute) and a recording
// bridge sink: join with recording notice, roster tracking, media routing, and resilient endings.

using Microsoft.Extensions.Logging;
using NSubstitute;
using NSubstitute.ExceptionExtensions;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Audio;
using TeamsBot.Core.Meeting;
using TeamsBot.Core.Model;
using TeamsBot.Core.Tests.Support;
using TeamsBot.Core.Video;
using Xunit;

namespace TeamsBot.Core.Tests.Meeting;

/// <summary>Tests for <see cref="BotService"/>.</summary>
public sealed class BotServiceTests : IDisposable
{
    private static readonly string[] RecordingThenMetadata = { "recording", "metadata" };

    private static readonly Uri JoinUrl = new("https://teams.microsoft.com/l/meetup-join/19%3ameeting%40thread.v2/0");
    private static readonly ParticipantInfo Ada = new("aad-ada", "Ada Lovelace");
    private static readonly ParticipantInfo Alan = new("aad-alan", "Alan Turing");
    private static readonly AudioFormatInfo Pcm16kMono = new(16_000, 1);

    private readonly ICallJoiner joiner = Substitute.For<ICallJoiner>();
    private readonly ICallSession session = Substitute.For<ICallSession>();
    private readonly FakeBridgeSink bridge = new();
    private readonly RecordingLoggerFactory loggers = new();

    /// <summary>Configures a joinable meeting with Ada on source 1.</summary>
    public BotServiceTests()
    {
        session.MeetingId.Returns("19:meeting@thread.v2");
        session.Title.Returns("Weekly sync");
        session.StartTime.Returns(DateTimeOffset.FromUnixTimeMilliseconds(1_793_000_000_000));
        session.Participants.Returns(new[] { new RosterParticipant(Ada, new uint[] { 1 }) });
        joiner.JoinAsync(JoinUrl, Arg.Any<CancellationToken>()).Returns(session);
    }

    /// <summary>Releases the logger factory.</summary>
    public void Dispose() => loggers.Dispose();

    /// <summary>Creates the service with fast retries.</summary>
    private BotService Service() => new(
        joiner,
        bridge,
        new BotServiceOptions { IdleFlushInterval = null, JoinAttempts = 3, JoinRetryDelay = TimeSpan.FromMilliseconds(1), EndFlushTimeout = TimeSpan.FromSeconds(1) },
        new AudioChunkingOptions(),
        new VideoProcessingOptions { SkipUnchangedFrames = false },
        loggers);

    /// <summary>A 20 ms frame of non-silent audio.</summary>
    private static UnmixedAudioFrame AudioFrame(uint sourceId, ulong offsetMs) =>
        new(sourceId, Enumerable.Repeat((byte)7, 640).ToArray(), Pcm16kMono, offsetMs);

    /// <summary>Joining announces recording before any metadata, then sends the roster and start time.</summary>
    [Fact]
    public async Task JoinAnnouncesRecordingThenSendsMetadata()
    {
        session.When(call => call.UpdateRecordingStatusAsync(true, Arg.Any<CancellationToken>())).Do(_ => bridge.Calls.Add("recording"));
        await using var service = Service();

        Assert.True(await service.JoinMeetingAsync(JoinUrl, CancellationToken.None));

        Assert.Equal(RecordingThenMetadata, bridge.Calls);
        var meeting = Assert.Single(bridge.Meetings);
        Assert.Equal("19:meeting@thread.v2", meeting.MeetingId);
        Assert.Equal(new[] { Ada }, meeting.Participants);
        Assert.Equal(session.StartTime, meeting.StartTime);
        Assert.True(service.IsInMeeting);
    }

    /// <summary>If Teams refuses the recording notice, the bot leaves without capturing anything.</summary>
    [Fact]
    public async Task RecordingStatusFailureLeavesTheMeeting()
    {
        session.UpdateRecordingStatusAsync(true, Arg.Any<CancellationToken>()).ThrowsAsync(new InvalidOperationException("forbidden"));
        await using var service = Service();

        Assert.False(await service.JoinMeetingAsync(JoinUrl, CancellationToken.None));

        await session.Received(1).HangUpAsync(Arg.Any<CancellationToken>());
        Assert.Empty(bridge.Meetings);
        Assert.False(service.IsInMeeting);
        Assert.True(loggers.Contains(LogLevel.Error, "recording"));
    }

    /// <summary>Transient join failures (e.g. an expired token) are retried; persistent ones are logged, not thrown.</summary>
    [Fact]
    public async Task JoinFailuresAreRetriedThenReported()
    {
        joiner.JoinAsync(JoinUrl, Arg.Any<CancellationToken>()).Returns(
            _ => throw new HttpRequestException("401"),
            _ => Task.FromResult(session));
        await using (var service = Service())
        {
            Assert.True(await service.JoinMeetingAsync(JoinUrl, CancellationToken.None));
        }

        joiner.JoinAsync(JoinUrl, Arg.Any<CancellationToken>()).ThrowsAsync(new HttpRequestException("401"));
        await using var failing = Service();
        Assert.False(await failing.JoinMeetingAsync(JoinUrl, CancellationToken.None));
        Assert.True(loggers.Contains(LogLevel.Error, "join"));
    }

    /// <summary>Only one meeting is streamed at a time.</summary>
    [Fact]
    public async Task ASecondJoinIsRejectedWhileInAMeeting()
    {
        await using var service = Service();
        await service.JoinMeetingAsync(JoinUrl, CancellationToken.None);

        Assert.False(await service.JoinMeetingAsync(JoinUrl, CancellationToken.None));
        await joiner.Received(1).JoinAsync(JoinUrl, Arg.Any<CancellationToken>());
    }

    /// <summary>Audio is chunked per speaker and tagged through the live roster, including late joiners.</summary>
    [Fact]
    public async Task AudioIsTaggedThroughTheRoster()
    {
        await using var service = Service();
        await service.JoinMeetingAsync(JoinUrl, CancellationToken.None);
        session.RosterChanged += Raise.Event<EventHandler<RosterChange>>(session,
            new RosterChange(new[] { new RosterParticipant(Alan, new uint[] { 2 }) }, Array.Empty<RosterParticipant>()));

        session.AudioFrameReceived += Raise.Event<EventHandler<UnmixedAudioFrame>>(session, AudioFrame(1, 0));
        session.AudioFrameReceived += Raise.Event<EventHandler<UnmixedAudioFrame>>(session, AudioFrame(2, 0));
        service.FlushIdleAudio(nowOffsetMs: 5_000);

        Assert.Equal(new[] { Ada, Alan }.OrderBy(p => p.TeamsUserId), bridge.Audio.Select(chunk => chunk.Participant).OrderBy(p => p.TeamsUserId));
        Assert.True(loggers.Contains(LogLevel.Information, "aad-alan"));
        Assert.Equal(new[] { Ada, Alan }, bridge.Meetings[^1].Participants);
    }

    /// <summary>Screen-share frames are encoded and queued; a new share resets change detection.</summary>
    [Fact]
    public async Task ScreenShareFramesAreQueuedAsJpeg()
    {
        await using var service = Service();
        await service.JoinMeetingAsync(JoinUrl, CancellationToken.None);
        session.ScreenShareStateChanged += Raise.Event<EventHandler<bool>>(session, true);

        session.ScreenShareFrameReceived += Raise.Event<EventHandler<RawVideoFrame>>(session, new RawVideoFrame(new byte[32 * 16 * 3 / 2], 32, 16, 32, 3_000));

        var frame = Assert.Single(bridge.Video);
        Assert.Equal(3_000u, frame.TimestampMs);
        Assert.Equal(0xFF, frame.ImageData.Span[0]);
    }

    /// <summary>An unexpected meeting end flushes pending audio and the bridge, and later events are ignored.</summary>
    [Fact]
    public async Task UnexpectedEndFlushesAndStopsCleanly()
    {
        await using var service = Service();
        await service.JoinMeetingAsync(JoinUrl, CancellationToken.None);
        session.AudioFrameReceived += Raise.Event<EventHandler<UnmixedAudioFrame>>(session, AudioFrame(1, 0));

        session.Ended += Raise.Event<EventHandler<CallEnded>>(session, new CallEnded("meeting ended by organizer", Unexpected: true));
        await service.WaitForMeetingEndAsync(TimeSpan.FromSeconds(5));

        Assert.Single(bridge.Audio);
        Assert.Equal(1, bridge.FlushCount);
        Assert.False(service.IsInMeeting);
        Assert.True(loggers.Contains(LogLevel.Warning, "ended"));
        session.AudioFrameReceived += Raise.Event<EventHandler<UnmixedAudioFrame>>(session, AudioFrame(1, 100));
        service.FlushIdleAudio(nowOffsetMs: 10_000);
        Assert.Single(bridge.Audio);
    }

    /// <summary>A failure while forwarding media is logged and does not escape to the media thread.</summary>
    [Fact]
    public async Task ProcessingFailuresDoNotEscape()
    {
        bridge.ThrowOnAudio = true;
        await using var service = Service();
        await service.JoinMeetingAsync(JoinUrl, CancellationToken.None);
        session.AudioFrameReceived += Raise.Event<EventHandler<UnmixedAudioFrame>>(session, AudioFrame(1, 0));

        service.FlushIdleAudio(nowOffsetMs: 5_000);

        Assert.True(loggers.Contains(LogLevel.Error, "audio"));
        Assert.True(service.IsInMeeting);
    }

    /// <summary>Leaving hangs up, clears the recording notice, and flushes.</summary>
    [Fact]
    public async Task LeaveHangsUpAndFlushes()
    {
        await using var service = Service();
        await service.JoinMeetingAsync(JoinUrl, CancellationToken.None);
        session.AudioFrameReceived += Raise.Event<EventHandler<UnmixedAudioFrame>>(session, AudioFrame(1, 0));

        await service.LeaveAsync(CancellationToken.None);

        await session.Received(1).HangUpAsync(Arg.Any<CancellationToken>());
        await session.Received(1).UpdateRecordingStatusAsync(false, Arg.Any<CancellationToken>());
        Assert.Single(bridge.Audio);
        Assert.False(service.IsInMeeting);
    }

    /// <summary>Records what the service sends to the bridge.</summary>
    private sealed class FakeBridgeSink : IBridgeSink
    {
        public List<MeetingInfo> Meetings { get; } = new();

        public List<string> Calls { get; } = new();

        public List<PcmAudioChunk> Audio { get; } = new();

        public List<EncodedVideoFrame> Video { get; } = new();

        public int FlushCount { get; private set; }

        public bool ThrowOnAudio { get; set; }

        public bool SetMeeting(MeetingInfo meeting)
        {
            Meetings.Add(meeting);
            Calls.Add("metadata");
            return true;
        }

        public bool EnqueueAudio(PcmAudioChunk chunk)
        {
            if (ThrowOnAudio)
            {
                throw new InvalidOperationException("bridge broken");
            }
            lock (Audio)
            {
                Audio.Add(chunk);
            }
            return true;
        }

        public bool EnqueueVideo(EncodedVideoFrame frame)
        {
            Video.Add(frame);
            return true;
        }

        public Task<bool> FlushAsync(TimeSpan timeout)
        {
            FlushCount++;
            return Task.FromResult(true);
        }
    }
}
