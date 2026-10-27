// Orchestrates one Teams meeting: joins it, announces recording, routes unmixed audio and
// screen-share frames through the processors to the bridge, keeps the roster (and the Rust
// app's metadata) current, and ends cleanly on leave or on an unexpected call end.
//
// Resilience: join failures (including expired tokens surfaced by the SDK) are retried, every
// event handler catches and logs its own failures so nothing escapes onto Graph media threads,
// and media arriving after the meeting ended is ignored. Logs contain IDs and counts only, never
// display names, titles, audio, or images.

using Microsoft.Extensions.Logging;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Audio;
using TeamsBot.Core.Model;
using TeamsBot.Core.Video;

namespace TeamsBot.Core.Meeting;

/// <summary>Lifecycle settings for <see cref="BotService"/>.</summary>
public sealed class BotServiceOptions
{
    /// <summary>How often idle speakers' audio is flushed; <c>null</c> disables the timer (callers use <see cref="BotService.FlushIdleAudio"/>).</summary>
    public TimeSpan? IdleFlushInterval { get; set; } = TimeSpan.FromSeconds(1);

    /// <summary>Join attempts before giving up (1–10).</summary>
    public int JoinAttempts { get; set; } = 3;

    /// <summary>Delay before the first retry; doubles per retry.</summary>
    public TimeSpan JoinRetryDelay { get; set; } = TimeSpan.FromSeconds(2);

    /// <summary>How long the end of a meeting waits for queued frames to reach the Rust app.</summary>
    public TimeSpan EndFlushTimeout { get; set; } = TimeSpan.FromSeconds(10);

    /// <summary>Checks the settings.</summary>
    /// <exception cref="ArgumentOutOfRangeException">A value is out of range.</exception>
    public void Validate()
    {
        if (IdleFlushInterval is { } interval && interval <= TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(IdleFlushInterval), "IdleFlushInterval must be positive.");
        }
        if (JoinAttempts is < 1 or > 10)
        {
            throw new ArgumentOutOfRangeException(nameof(JoinAttempts), "JoinAttempts must be between 1 and 10.");
        }
        if (JoinRetryDelay < TimeSpan.Zero || EndFlushTimeout < TimeSpan.Zero)
        {
            throw new ArgumentOutOfRangeException(nameof(JoinRetryDelay), "Delays must not be negative.");
        }
    }
}

/// <summary>Streams one Teams meeting at a time to the Rust app.</summary>
public sealed partial class BotService : IAsyncDisposable
{
    private readonly ICallJoiner callJoiner;
    private readonly IBridgeSink bridge;
    private readonly BotServiceOptions options;
    private readonly AudioChunkingOptions audioOptions;
    private readonly VideoProcessingOptions videoOptions;
    private readonly ILoggerFactory loggerFactory;
    private readonly ILogger<BotService> logger;

    /// <summary>1 while a join is in progress or a meeting is active.</summary>
    private int busy;

    private volatile ActiveMeeting? activeMeeting;

    /// <summary>Completes when the most recent meeting finished ending.</summary>
    private Task lastMeetingEnd = Task.CompletedTask;

    /// <summary>Creates the service.</summary>
    /// <param name="callJoiner">Joins meetings (the Graph adapter).</param>
    /// <param name="bridge">Receives metadata and media (the WebSocket client).</param>
    /// <param name="options">Lifecycle settings.</param>
    /// <param name="audioOptions">Per-speaker chunking settings.</param>
    /// <param name="videoOptions">Screen-share sampling and JPEG settings.</param>
    /// <param name="loggerFactory">Creates the service's and processors' loggers.</param>
    /// <exception cref="ArgumentOutOfRangeException">An option is out of range.</exception>
    /// <remarks>Side effects: None until a meeting is joined.</remarks>
    public BotService(
        ICallJoiner callJoiner,
        IBridgeSink bridge,
        BotServiceOptions options,
        AudioChunkingOptions audioOptions,
        VideoProcessingOptions videoOptions,
        ILoggerFactory loggerFactory)
    {
        ArgumentNullException.ThrowIfNull(callJoiner);
        ArgumentNullException.ThrowIfNull(bridge);
        ArgumentNullException.ThrowIfNull(options);
        ArgumentNullException.ThrowIfNull(audioOptions);
        ArgumentNullException.ThrowIfNull(videoOptions);
        ArgumentNullException.ThrowIfNull(loggerFactory);
        options.Validate();
        audioOptions.Validate();
        videoOptions.Validate();
        this.callJoiner = callJoiner;
        this.bridge = bridge;
        this.options = options;
        this.audioOptions = audioOptions;
        this.videoOptions = videoOptions;
        this.loggerFactory = loggerFactory;
        logger = loggerFactory.CreateLogger<BotService>();
    }

    /// <summary>Whether a meeting is currently being streamed.</summary>
    public bool IsInMeeting => activeMeeting is not null;

    /// <summary>Joins a meeting, announces recording, and starts streaming it.</summary>
    /// <param name="joinUrl">The meeting's Teams join link (never logged).</param>
    /// <param name="cancellationToken">Cancels the join.</param>
    /// <returns><c>true</c> when streaming started; <c>false</c> when busy, when every join attempt failed, or when Teams refused the recording status.</returns>
    /// <exception cref="OperationCanceledException"><paramref name="cancellationToken"/> was cancelled.</exception>
    /// <remarks>Side effects: Calls Graph (join, updateRecordingStatus, hang-up on refusal); sends metadata to the bridge; starts the idle-flush timer.</remarks>
    public async Task<bool> JoinMeetingAsync(Uri joinUrl, CancellationToken cancellationToken)
    {
        ArgumentNullException.ThrowIfNull(joinUrl);
        if (Interlocked.CompareExchange(ref busy, 1, 0) != 0)
        {
            LogBusy();
            return false;
        }

        var started = false;
        try
        {
            var session = await JoinWithRetriesAsync(joinUrl, cancellationToken).ConfigureAwait(false);
            if (session is null)
            {
                return false;
            }

            try
            {
                await session.UpdateRecordingStatusAsync(true, cancellationToken).ConfigureAwait(false);
            }
            catch (Exception failure) when (failure is not OperationCanceledException)
            {
                LogRecordingRefused(failure.GetType().Name);
                await HangUpQuietlyAsync(session).ConfigureAwait(false);
                return false;
            }

            StartStreaming(session);
            started = true;
            return true;
        }
        finally
        {
            if (!started)
            {
                Volatile.Write(ref busy, 0);
            }
        }
    }

    /// <summary>Leaves the current meeting (no-op when not in one).</summary>
    /// <param name="cancellationToken">Cancels the Graph requests (the local flush still happens).</param>
    /// <returns>A task that completes when the meeting ended.</returns>
    /// <remarks>Side effects: Flushes pending audio to the bridge, clears the recording status, hangs up, and waits for the bridge to drain.</remarks>
    public async Task LeaveAsync(CancellationToken cancellationToken)
    {
        var meeting = Detach();
        if (meeting is null)
        {
            return;
        }

        var end = EndAsync(meeting, new CallEnded("left by the bot", Unexpected: false), hangUp: true, cancellationToken);
        lastMeetingEnd = end;
        await end.ConfigureAwait(false);
    }

    /// <summary>Waits for the most recent meeting to finish ending (after <see cref="ICallSession.Ended"/>).</summary>
    /// <param name="timeout">How long to wait.</param>
    /// <returns><c>true</c> when no meeting is ending any more.</returns>
    public async Task<bool> WaitForMeetingEndAsync(TimeSpan timeout)
    {
        var end = lastMeetingEnd;
        var winner = await Task.WhenAny(end, Task.Delay(timeout)).ConfigureAwait(false);
        return winner == end;
    }

    /// <summary>Sends audio of speakers who have been silent long enough (run periodically).</summary>
    /// <param name="nowOffsetMs">The current time in milliseconds since the meeting start.</param>
    /// <remarks>Side effects: Queues completed chunks on the bridge; logs (never throws) failures.</remarks>
    public void FlushIdleAudio(ulong nowOffsetMs)
    {
        var meeting = activeMeeting;
        if (meeting is null)
        {
            return;
        }
        try
        {
            Forward(meeting.Audio.FlushIdle(nowOffsetMs));
        }
        catch (Exception failure)
        {
            LogAudioFailed(failure.GetType().Name);
        }
    }

    /// <summary>Leaves any active meeting and waits for an ending meeting to finish.</summary>
    /// <returns>A task that completes when everything stopped.</returns>
    /// <remarks>Side effects: As <see cref="LeaveAsync"/>.</remarks>
    public async ValueTask DisposeAsync()
    {
        try
        {
            await LeaveAsync(CancellationToken.None).ConfigureAwait(false);
            await lastMeetingEnd.ConfigureAwait(false);
        }
        catch (Exception failure)
        {
            LogEndFailed(failure.GetType().Name);
        }
    }

    /// <summary>Joins with exponential retries; returns <c>null</c> after the last failure.</summary>
    private async Task<ICallSession?> JoinWithRetriesAsync(Uri joinUrl, CancellationToken cancellationToken)
    {
        var retryDelay = options.JoinRetryDelay;
        for (var attempt = 1; attempt <= options.JoinAttempts; attempt++)
        {
            try
            {
                return await callJoiner.JoinAsync(joinUrl, cancellationToken).ConfigureAwait(false);
            }
            catch (Exception failure) when (failure is not OperationCanceledException || !cancellationToken.IsCancellationRequested)
            {
                LogJoinAttemptFailed(attempt, options.JoinAttempts, failure.GetType().Name);
            }

            if (attempt < options.JoinAttempts)
            {
                await Task.Delay(retryDelay, cancellationToken).ConfigureAwait(false);
                retryDelay *= 2;
            }
        }

        LogJoinFailed(options.JoinAttempts);
        return null;
    }

    /// <summary>Creates the per-meeting state, subscribes to the session, and announces the meeting.</summary>
    private void StartStreaming(ICallSession session)
    {
        var roster = new RosterDirectory();
        roster.AddOrUpdate(session.Participants);
        var meeting = new ActiveMeeting(
            session,
            roster,
            new AudioProcessor(audioOptions, roster, loggerFactory.CreateLogger<AudioProcessor>()),
            new VideoProcessor(videoOptions, loggerFactory.CreateLogger<VideoProcessor>()));

        meeting.RosterHandler = (_, change) => OnRosterChanged(meeting, change);
        meeting.AudioHandler = (_, frame) => OnAudioFrame(meeting, frame);
        meeting.ScreenShareStateHandler = (_, sharing) => OnScreenShareState(meeting, sharing);
        meeting.ScreenShareFrameHandler = (_, frame) => OnScreenShareFrame(meeting, frame);
        meeting.EndedHandler = (_, ended) => OnEnded(meeting, ended);
        session.RosterChanged += meeting.RosterHandler;
        session.AudioFrameReceived += meeting.AudioHandler;
        session.ScreenShareStateChanged += meeting.ScreenShareStateHandler;
        session.ScreenShareFrameReceived += meeting.ScreenShareFrameHandler;
        session.Ended += meeting.EndedHandler;

        activeMeeting = meeting;
        Announce(meeting);
        if (options.IdleFlushInterval is { } interval)
        {
            meeting.IdleFlushTimer = new Timer(
                _ => FlushIdleAudio(OffsetNow(meeting)),
                null,
                interval,
                interval);
        }
        LogMeetingJoined(session.MeetingId, roster.Snapshot().Count);
    }

    /// <summary>Sends (or re-sends) the meeting metadata with the current roster.</summary>
    private void Announce(ActiveMeeting meeting)
    {
        var info = new MeetingInfo(meeting.Session.MeetingId, meeting.Session.Title, meeting.Session.StartTime, meeting.Roster.Snapshot());
        if (!bridge.SetMeeting(info))
        {
            LogMetadataRejected(meeting.Session.MeetingId);
        }
    }

    /// <summary>Updates the roster, logs joins and departures by Teams user ID, and re-announces the meeting.</summary>
    private void OnRosterChanged(ActiveMeeting meeting, RosterChange change)
    {
        if (!ReferenceEquals(activeMeeting, meeting))
        {
            return;
        }
        try
        {
            var joined = meeting.Roster.AddOrUpdate(change.Joined);
            var left = meeting.Roster.Remove(change.Left);
            foreach (var entry in joined)
            {
                LogParticipantJoined(entry.Participant.TeamsUserId);
            }
            foreach (var entry in left)
            {
                LogParticipantLeft(entry.Participant.TeamsUserId);
            }
            if (change.Joined.Count > 0 || left.Count > 0)
            {
                // The Rust app resumes a meeting it already knows, so this only refreshes identities.
                Announce(meeting);
            }
        }
        catch (Exception failure)
        {
            LogRosterFailed(failure.GetType().Name);
        }
    }

    /// <summary>Chunks one unmixed buffer and queues completed chunks.</summary>
    private void OnAudioFrame(ActiveMeeting meeting, UnmixedAudioFrame frame)
    {
        if (!ReferenceEquals(activeMeeting, meeting))
        {
            return;
        }
        try
        {
            Forward(meeting.Audio.ProcessFrame(frame));
        }
        catch (Exception failure)
        {
            LogAudioFailed(failure.GetType().Name);
        }
    }

    /// <summary>Resets change detection when a share starts, so its first frame is always sent.</summary>
    private void OnScreenShareState(ActiveMeeting meeting, bool sharing)
    {
        if (!ReferenceEquals(activeMeeting, meeting))
        {
            return;
        }
        lock (meeting.Video)
        {
            meeting.Video.Reset();
        }
        if (sharing)
        {
            LogScreenShareStarted();
        }
        else
        {
            LogScreenShareStopped();
        }
    }

    /// <summary>Samples and encodes one screen-share frame (at most VideoFps per second) and queues it.</summary>
    private void OnScreenShareFrame(ActiveMeeting meeting, RawVideoFrame frame)
    {
        if (!ReferenceEquals(activeMeeting, meeting))
        {
            return;
        }
        try
        {
            EncodedVideoFrame? encoded;
            lock (meeting.Video)
            {
                encoded = meeting.Video.ProcessFrame(frame);
            }
            if (encoded is not null)
            {
                bridge.EnqueueVideo(encoded);
            }
        }
        catch (Exception failure)
        {
            LogVideoFailed(failure.GetType().Name);
        }
    }

    /// <summary>Ends the meeting off the notification thread when Teams ended the call.</summary>
    private void OnEnded(ActiveMeeting meeting, CallEnded ended)
    {
        if (!ReferenceEquals(Interlocked.CompareExchange(ref activeMeeting, null, meeting), meeting))
        {
            return;
        }
        lastMeetingEnd = Task.Run(() => EndAsync(meeting, ended, hangUp: false, CancellationToken.None));
    }

    /// <summary>Detaches the active meeting so that no further events are processed.</summary>
    private ActiveMeeting? Detach()
    {
        var meeting = activeMeeting;
        while (meeting is not null)
        {
            var previous = Interlocked.CompareExchange(ref activeMeeting, null, meeting);
            if (ReferenceEquals(previous, meeting))
            {
                return meeting;
            }
            meeting = previous;
        }
        return null;
    }

    /// <summary>Unsubscribes, flushes all audio, optionally clears recording and hangs up, and drains the bridge. Never throws.</summary>
    private async Task EndAsync(ActiveMeeting meeting, CallEnded ended, bool hangUp, CancellationToken cancellationToken)
    {
        try
        {
            var session = meeting.Session;
            session.RosterChanged -= meeting.RosterHandler;
            session.AudioFrameReceived -= meeting.AudioHandler;
            session.ScreenShareStateChanged -= meeting.ScreenShareStateHandler;
            session.ScreenShareFrameReceived -= meeting.ScreenShareFrameHandler;
            session.Ended -= meeting.EndedHandler;
            if (meeting.IdleFlushTimer is { } timer)
            {
                await timer.DisposeAsync().ConfigureAwait(false);
            }

            try
            {
                Forward(meeting.Audio.FlushAll());
            }
            catch (Exception failure)
            {
                LogAudioFailed(failure.GetType().Name);
            }

            if (hangUp)
            {
                try
                {
                    await session.UpdateRecordingStatusAsync(false, cancellationToken).ConfigureAwait(false);
                }
                catch (Exception failure)
                {
                    LogEndFailed(failure.GetType().Name);
                }
                await HangUpQuietlyAsync(session).ConfigureAwait(false);
            }

            if (ended.Unexpected)
            {
                LogMeetingEndedUnexpectedly(session.MeetingId, ended.Reason);
            }
            else
            {
                LogMeetingEnded(session.MeetingId, ended.Reason);
            }

            if (!await bridge.FlushAsync(options.EndFlushTimeout).ConfigureAwait(false))
            {
                LogFlushIncomplete(options.EndFlushTimeout.TotalSeconds);
            }
        }
        catch (Exception failure)
        {
            LogEndFailed(failure.GetType().Name);
        }
        finally
        {
            Volatile.Write(ref busy, 0);
        }
    }

    /// <summary>Hangs up, logging instead of throwing.</summary>
    private async Task HangUpQuietlyAsync(ICallSession session)
    {
        try
        {
            await session.HangUpAsync(CancellationToken.None).ConfigureAwait(false);
        }
        catch (Exception failure)
        {
            LogEndFailed(failure.GetType().Name);
        }
    }

    /// <summary>Queues chunks on the bridge.</summary>
    private void Forward(IReadOnlyList<PcmAudioChunk> chunks)
    {
        foreach (var chunk in chunks)
        {
            bridge.EnqueueAudio(chunk);
        }
    }

    /// <summary>Milliseconds since the meeting start (0 before it).</summary>
    private static ulong OffsetNow(ActiveMeeting meeting)
    {
        var elapsed = DateTimeOffset.UtcNow - meeting.Session.StartTime;
        return elapsed <= TimeSpan.Zero ? 0 : (ulong)elapsed.TotalMilliseconds;
    }

    [LoggerMessage(Level = LogLevel.Warning, Message = "Join request ignored: the bot is already in a meeting")]
    private partial void LogBusy();

    [LoggerMessage(Level = LogLevel.Warning, Message = "Join attempt {Attempt}/{MaximumAttempts} failed ({ErrorType})")]
    private partial void LogJoinAttemptFailed(int attempt, int maximumAttempts, string errorType);

    [LoggerMessage(Level = LogLevel.Error, Message = "Could not join the meeting after {Attempts} attempts")]
    private partial void LogJoinFailed(int attempts);

    [LoggerMessage(Level = LogLevel.Error, Message = "Teams refused the recording status ({ErrorType}); leaving without capturing media")]
    private partial void LogRecordingRefused(string errorType);

    [LoggerMessage(Level = LogLevel.Information, Message = "Joined meeting {MeetingId} with {ParticipantCount} participants; recording announced")]
    private partial void LogMeetingJoined(string meetingId, int participantCount);

    [LoggerMessage(Level = LogLevel.Warning, Message = "The bridge rejected the metadata of meeting {MeetingId}")]
    private partial void LogMetadataRejected(string meetingId);

    [LoggerMessage(Level = LogLevel.Information, Message = "Participant {TeamsUserId} joined")]
    private partial void LogParticipantJoined(string teamsUserId);

    [LoggerMessage(Level = LogLevel.Information, Message = "Participant {TeamsUserId} left")]
    private partial void LogParticipantLeft(string teamsUserId);

    [LoggerMessage(Level = LogLevel.Error, Message = "Roster update failed ({ErrorType})")]
    private partial void LogRosterFailed(string errorType);

    [LoggerMessage(Level = LogLevel.Error, Message = "Forwarding audio failed ({ErrorType}); the meeting continues")]
    private partial void LogAudioFailed(string errorType);

    [LoggerMessage(Level = LogLevel.Error, Message = "Forwarding a screen-share frame failed ({ErrorType}); the meeting continues")]
    private partial void LogVideoFailed(string errorType);

    [LoggerMessage(Level = LogLevel.Information, Message = "Screen share started")]
    private partial void LogScreenShareStarted();

    [LoggerMessage(Level = LogLevel.Information, Message = "Screen share stopped")]
    private partial void LogScreenShareStopped();

    [LoggerMessage(Level = LogLevel.Warning, Message = "Meeting {MeetingId} ended unexpectedly: {Reason}")]
    private partial void LogMeetingEndedUnexpectedly(string meetingId, string reason);

    [LoggerMessage(Level = LogLevel.Information, Message = "Meeting {MeetingId} ended: {Reason}")]
    private partial void LogMeetingEnded(string meetingId, string reason);

    [LoggerMessage(Level = LogLevel.Warning, Message = "The bridge did not drain within {Seconds} s after the meeting ended; buffered frames are sent after reconnecting")]
    private partial void LogFlushIncomplete(double seconds);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Ending the meeting hit an error ({ErrorType}); continuing")]
    private partial void LogEndFailed(string errorType);

    /// <summary>State of the meeting being streamed.</summary>
    private sealed class ActiveMeeting(ICallSession session, RosterDirectory roster, AudioProcessor audio, VideoProcessor video)
    {
        public ICallSession Session { get; } = session;

        public RosterDirectory Roster { get; } = roster;

        public AudioProcessor Audio { get; } = audio;

        public VideoProcessor Video { get; } = video;

        public Timer? IdleFlushTimer { get; set; }

        public EventHandler<RosterChange>? RosterHandler { get; set; }

        public EventHandler<UnmixedAudioFrame>? AudioHandler { get; set; }

        public EventHandler<bool>? ScreenShareStateHandler { get; set; }

        public EventHandler<RawVideoFrame>? ScreenShareFrameHandler { get; set; }

        public EventHandler<CallEnded>? EndedHandler { get; set; }
    }
}
