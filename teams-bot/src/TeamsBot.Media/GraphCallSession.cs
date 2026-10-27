// Adapts one joined Graph call (ICall + ILocalMediaSession) to TeamsBot.Core's ICallSession:
// roster updates become RosterChange events (participants keyed by Azure AD object ID, with their
// audio media source IDs), unmixed audio buffers and VBSS (screen-share) NV12 frames are copied
// out of native memory into managed frames, and the call's termination becomes Ended.
//
// Every SDK callback catches its own exceptions and always disposes the native media buffer, so
// a faulty frame can neither crash the media platform nor leak native memory.

using System.Diagnostics;
using System.Runtime.InteropServices;
using Microsoft.Extensions.Logging;
using Microsoft.Graph.Communications.Calls;
using Microsoft.Graph.Communications.Calls.Media;
using Microsoft.Graph.Communications.Resources;
using Microsoft.Graph.Models;
using Microsoft.Skype.Bots.Media;
using TeamsBot.Core.Abstractions;
using LogLevel = Microsoft.Extensions.Logging.LogLevel;

namespace TeamsBot.Media;

/// <summary>A joined Teams meeting with app-hosted media.</summary>
internal sealed partial class GraphCallSession : ICallSession
{
    /// <summary>The audio format the audio socket is configured for (16 kHz, 16-bit, mono).</summary>
    private static readonly AudioFormatInfo SocketAudioFormat = new(16_000, 1);

    private readonly ICall call;
    private readonly ILocalMediaSession mediaSession;
    private readonly string applicationId;
    private readonly ILogger logger;

    /// <summary>Measures frame offsets from the moment the call was established.</summary>
    private readonly Stopwatch sinceStart = new();

    private readonly object gate = new();

    /// <summary>Participants currently represented in the roster, keyed by Graph participant ID.</summary>
    private readonly Dictionary<string, RosterParticipant> rosterByParticipantId = new(StringComparer.Ordinal);

    /// <summary>The VBSS source currently subscribed to, if any.</summary>
    private uint? subscribedScreenShareSource;

    private int ended;
    private volatile bool hangingUp;

    /// <summary>Wraps an established call.</summary>
    /// <param name="call">The Graph call.</param>
    /// <param name="mediaSession">Its local media session (audio + VBSS sockets).</param>
    /// <param name="applicationId">The bot's app ID (excluded from the roster).</param>
    /// <param name="logger">Receives adapter events (IDs only).</param>
    /// <remarks>Side effects: Subscribes to call, roster, and media events.</remarks>
    public GraphCallSession(ICall call, ILocalMediaSession mediaSession, string applicationId, ILogger logger)
    {
        this.call = call;
        this.mediaSession = mediaSession;
        this.applicationId = applicationId;
        this.logger = logger;
        MeetingId = call.Resource.ChatInfo?.ThreadId ?? call.Id;
        Title = call.Resource.Subject ?? string.Empty;
        StartTime = DateTimeOffset.UtcNow;
        sinceStart.Start();

        foreach (var participant in call.Participants)
        {
            Track(participant);
        }

        call.OnUpdated += OnCallUpdated;
        call.Participants.OnUpdated += OnParticipantsUpdated;
        mediaSession.AudioSocket.AudioMediaReceived += OnAudioMediaReceived;
        if (mediaSession.VbssSocket is { } vbss)
        {
            vbss.VideoMediaReceived += OnScreenShareMediaReceived;
            vbss.VideoReceiveStatusChanged += OnScreenShareStatusChanged;
        }
        UpdateScreenShareSubscription();
    }

    /// <inheritdoc />
    public string MeetingId { get; }

    /// <inheritdoc />
    public string Title { get; }

    /// <inheritdoc />
    public DateTimeOffset StartTime { get; }

    /// <inheritdoc />
    public IReadOnlyCollection<RosterParticipant> Participants
    {
        get
        {
            lock (gate)
            {
                return rosterByParticipantId.Values.ToArray();
            }
        }
    }

    /// <inheritdoc />
    public event EventHandler<RosterChange>? RosterChanged;

    /// <inheritdoc />
    public event EventHandler<UnmixedAudioFrame>? AudioFrameReceived;

    /// <inheritdoc />
    public event EventHandler<bool>? ScreenShareStateChanged;

    /// <inheritdoc />
    public event EventHandler<RawVideoFrame>? ScreenShareFrameReceived;

    /// <inheritdoc />
    public event EventHandler<CallEnded>? Ended;

    /// <inheritdoc />
    /// <remarks>Side effects: Calls Graph <c>updateRecordingStatus</c>.</remarks>
    public Task UpdateRecordingStatusAsync(bool recording, CancellationToken cancellationToken) =>
        call.UpdateRecordingStatusAsync(recording ? RecordingStatus.Recording : RecordingStatus.NotRecording, cancellationToken);

    /// <inheritdoc />
    /// <remarks>Side effects: Deletes the Graph call; the resulting termination is reported as an expected end.</remarks>
    public async Task HangUpAsync(CancellationToken cancellationToken)
    {
        hangingUp = true;
        await call.DeleteAsync(false, cancellationToken).ConfigureAwait(false);
        RaiseEnded("hung up by the bot", unexpected: false);
    }

    /// <summary>Raises <see cref="Ended"/> when Graph reports the call terminated.</summary>
    private void OnCallUpdated(ICall sender, ResourceEventArgs<Call> args)
    {
        try
        {
            if (args.NewResource?.State == CallState.Terminated)
            {
                var result = args.NewResource.ResultInfo;
                var reason = result is null ? "call terminated" : $"call terminated (code {result.Code}, subcode {result.Subcode})";
                RaiseEnded(reason, unexpected: !hangingUp);
            }
        }
        catch (Exception failure)
        {
            LogCallbackFailed("call update", failure.GetType().Name);
        }
    }

    /// <summary>Translates participant additions, removals, and updates into roster changes.</summary>
    private void OnParticipantsUpdated(IParticipantCollection sender, CollectionEventArgs<IParticipant> args)
    {
        try
        {
            var joined = new List<RosterParticipant>();
            var left = new List<RosterParticipant>();
            foreach (var participant in args.AddedResources ?? Array.Empty<IParticipant>())
            {
                if (Track(participant) is { } entry)
                {
                    joined.Add(entry);
                }
            }
            foreach (var participant in args.RemovedResources ?? Array.Empty<IParticipant>())
            {
                participant.OnUpdated -= OnParticipantUpdated;
                lock (gate)
                {
                    if (rosterByParticipantId.Remove(participant.Id, out var entry))
                    {
                        left.Add(entry);
                    }
                }
            }
            UpdateScreenShareSubscription();
            if (joined.Count > 0 || left.Count > 0)
            {
                RosterChanged?.Invoke(this, new RosterChange(joined, left));
            }
        }
        catch (Exception failure)
        {
            LogCallbackFailed("roster update", failure.GetType().Name);
        }
    }

    /// <summary>Re-reads a participant whose media streams changed (e.g. started sharing).</summary>
    private void OnParticipantUpdated(IParticipant sender, ResourceEventArgs<Participant> args)
    {
        try
        {
            if (Track(sender) is { } entry)
            {
                RosterChanged?.Invoke(this, new RosterChange(new[] { entry }, Array.Empty<RosterParticipant>()));
            }
            UpdateScreenShareSubscription();
        }
        catch (Exception failure)
        {
            LogCallbackFailed("participant update", failure.GetType().Name);
        }
    }

    /// <summary>Records a participant (skipping the bot itself and the lobby) and returns its entry.</summary>
    private RosterParticipant? Track(IParticipant participant)
    {
        var resource = participant.Resource;
        if (resource is null || resource.IsInLobby == true || resource.Info?.Identity?.Application?.Id == applicationId)
        {
            return null;
        }

        var entry = new RosterParticipant(ParticipantMapping.ToParticipantInfo(participant.Id, resource), ParticipantMapping.AudioSourceIds(resource));
        lock (gate)
        {
            var isNew = !rosterByParticipantId.ContainsKey(participant.Id);
            rosterByParticipantId[participant.Id] = entry;
            if (isNew)
            {
                participant.OnUpdated += OnParticipantUpdated;
            }
        }
        return entry;
    }

    /// <summary>Subscribes the VBSS socket to whoever is sharing (Teams allows one sharer at a time).</summary>
    private void UpdateScreenShareSubscription()
    {
        if (mediaSession.VbssSocket is not { } vbss)
        {
            return;
        }

        uint? sharer = null;
        foreach (var participant in call.Participants)
        {
            if (participant.Resource is { } resource && ParticipantMapping.ScreenShareSourceId(resource) is { } sourceId)
            {
                sharer = sourceId;
                break;
            }
        }

        lock (gate)
        {
            if (sharer == subscribedScreenShareSource)
            {
                return;
            }
            subscribedScreenShareSource = sharer;
        }

        if (sharer is { } source)
        {
            vbss.Subscribe(VideoResolution.HD1080p, source);
            LogScreenShareSubscribed(source);
        }
        else
        {
            vbss.Unsubscribe();
        }
    }

    /// <summary>Copies each speaker's unmixed 20 ms buffer out of native memory.</summary>
    private void OnAudioMediaReceived(object? sender, AudioMediaReceivedEventArgs args)
    {
        var buffer = args.Buffer;
        try
        {
            var offsetMs = (ulong)sinceStart.ElapsedMilliseconds;
            if (buffer.UnmixedAudioBuffers is not { } unmixed)
            {
                return;
            }
            foreach (var speaker in unmixed)
            {
                if (speaker.Length <= 0 || speaker.Data == IntPtr.Zero)
                {
                    continue;
                }
                var pcm = new byte[speaker.Length];
                Marshal.Copy(speaker.Data, pcm, 0, pcm.Length);
                AudioFrameReceived?.Invoke(this, new UnmixedAudioFrame(speaker.ActiveSpeakerId, pcm, SocketAudioFormat, offsetMs));
            }
        }
        catch (Exception failure)
        {
            LogCallbackFailed("audio", failure.GetType().Name);
        }
        finally
        {
            buffer.Dispose();
        }
    }

    /// <summary>Copies one NV12 screen-share frame out of native memory.</summary>
    private void OnScreenShareMediaReceived(object? sender, VideoMediaReceivedEventArgs args)
    {
        var buffer = args.Buffer;
        try
        {
            var format = buffer.VideoFormat;
            if (format.VideoColorFormat != VideoColorFormat.NV12 || buffer.Data == IntPtr.Zero || buffer.Length <= 0)
            {
                return;
            }
            var data = new byte[buffer.Length];
            Marshal.Copy(buffer.Data, data, 0, data.Length);
            // The media platform delivers tightly packed NV12, so the luma stride equals the width.
            ScreenShareFrameReceived?.Invoke(this, new RawVideoFrame(data, format.Width, format.Height, format.Width, (ulong)sinceStart.ElapsedMilliseconds));
        }
        catch (Exception failure)
        {
            LogCallbackFailed("screen share", failure.GetType().Name);
        }
        finally
        {
            buffer.Dispose();
        }
    }

    /// <summary>Reports a share starting or stopping.</summary>
    private void OnScreenShareStatusChanged(object? sender, VideoReceiveStatusChangedEventArgs args)
    {
        try
        {
            ScreenShareStateChanged?.Invoke(this, args.MediaReceiveStatus == MediaReceiveStatus.Active);
        }
        catch (Exception failure)
        {
            LogCallbackFailed("screen share status", failure.GetType().Name);
        }
    }

    /// <summary>Raises <see cref="Ended"/> once and unsubscribes from the SDK.</summary>
    private void RaiseEnded(string reason, bool unexpected)
    {
        if (Interlocked.Exchange(ref ended, 1) != 0)
        {
            return;
        }
        call.OnUpdated -= OnCallUpdated;
        call.Participants.OnUpdated -= OnParticipantsUpdated;
        mediaSession.AudioSocket.AudioMediaReceived -= OnAudioMediaReceived;
        if (mediaSession.VbssSocket is { } vbss)
        {
            vbss.VideoMediaReceived -= OnScreenShareMediaReceived;
            vbss.VideoReceiveStatusChanged -= OnScreenShareStatusChanged;
        }
        Ended?.Invoke(this, new CallEnded(reason, unexpected));
    }

    [LoggerMessage(Level = LogLevel.Error, Message = "Teams {Callback} callback failed ({ErrorType}); continuing")]
    private partial void LogCallbackFailed(string callback, string errorType);

    [LoggerMessage(Level = LogLevel.Information, Message = "Subscribed to the screen share from media source {SourceId}")]
    private partial void LogScreenShareSubscribed(uint sourceId);
}
