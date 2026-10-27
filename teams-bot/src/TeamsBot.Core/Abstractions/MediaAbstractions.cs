// Seams between the portable core and the Teams Real-Time Media SDK. TeamsBot.Media implements
// them over the Graph Communications SDK on Windows; tests substitute them.

using System.Diagnostics.CodeAnalysis;
using TeamsBot.Core.Model;

namespace TeamsBot.Core.Abstractions;

/// <summary>The PCM layout of an audio buffer (always signed 16-bit little-endian).</summary>
/// <param name="SampleRateHz">Samples per second per channel.</param>
/// <param name="Channels">Interleaved channel count.</param>
public readonly record struct AudioFormatInfo(int SampleRateHz, int Channels);

/// <summary>One unmixed (single-speaker) audio buffer delivered by the media platform.</summary>
/// <param name="SourceId">The speaker's media source ID (MSI); resolved through <see cref="IParticipantDirectory"/>.</param>
/// <param name="Pcm">Signed 16-bit little-endian PCM in <paramref name="Format"/>; never logged.</param>
/// <param name="Format">The PCM layout.</param>
/// <param name="OffsetMs">Buffer start in milliseconds since the meeting start.</param>
public sealed record UnmixedAudioFrame(uint SourceId, ReadOnlyMemory<byte> Pcm, AudioFormatInfo Format, ulong OffsetMs);

/// <summary>Resolves media source IDs to roster participants.</summary>
public interface IParticipantDirectory
{
    /// <summary>Looks up the participant that owns a media source.</summary>
    /// <param name="mediaSourceId">The MSI from an audio buffer.</param>
    /// <param name="participant">The participant, when known.</param>
    /// <returns><c>true</c> when the source belongs to a known participant.</returns>
    bool TryResolve(uint mediaSourceId, [NotNullWhen(true)] out ParticipantInfo? participant);
}

/// <summary>One raw screen-share frame in NV12 (a Y plane, then an interleaved UV plane at half resolution).</summary>
/// <param name="Data">The planes: <paramref name="Height"/> luma rows then <paramref name="Height"/>/2 chroma rows, each <paramref name="Stride"/> bytes; never logged.</param>
/// <param name="Width">Visible width in pixels (even).</param>
/// <param name="Height">Visible height in pixels (even).</param>
/// <param name="Stride">Bytes per row, at least <paramref name="Width"/>.</param>
/// <param name="OffsetMs">Capture time in milliseconds since the meeting start.</param>
public sealed record RawVideoFrame(ReadOnlyMemory<byte> Data, int Width, int Height, int Stride, ulong OffsetMs);

/// <summary>A roster entry: the participant and the media source IDs (MSIs) of its audio.</summary>
/// <param name="Participant">Who the participant is.</param>
/// <param name="AudioSourceIds">MSIs that tag this participant's unmixed audio.</param>
public sealed record RosterParticipant(ParticipantInfo Participant, IReadOnlyCollection<uint> AudioSourceIds);

/// <summary>Participants that joined or left.</summary>
/// <param name="Joined">New or updated entries.</param>
/// <param name="Left">Departed entries.</param>
public sealed record RosterChange(IReadOnlyList<RosterParticipant> Joined, IReadOnlyList<RosterParticipant> Left);

/// <summary>Why a call ended.</summary>
/// <param name="Reason">A short, non-sensitive description (e.g. the Graph result code).</param>
/// <param name="Unexpected"><c>true</c> when the bot did not hang up itself.</param>
public sealed record CallEnded(string Reason, bool Unexpected);

/// <summary>A joined Teams meeting with app-hosted media (implemented over the Graph SDK in TeamsBot.Media).</summary>
public interface ICallSession
{
    /// <summary>The Teams meeting (thread) ID.</summary>
    string MeetingId { get; }

    /// <summary>The meeting subject (may be empty; never logged).</summary>
    string Title { get; }

    /// <summary>When the bot joined; frame offsets are relative to it.</summary>
    DateTimeOffset StartTime { get; }

    /// <summary>The roster at join time.</summary>
    IReadOnlyCollection<RosterParticipant> Participants { get; }

    /// <summary>Raised when participants join or leave.</summary>
    event EventHandler<RosterChange>? RosterChanged;

    /// <summary>Raised for each unmixed audio buffer (on a media thread; handlers must not block).</summary>
    event EventHandler<UnmixedAudioFrame>? AudioFrameReceived;

    /// <summary>Raised when a screen share starts (<c>true</c>) or stops (<c>false</c>).</summary>
    event EventHandler<bool>? ScreenShareStateChanged;

    /// <summary>Raised for each screen-share frame (on a media thread; handlers must not block).</summary>
    event EventHandler<RawVideoFrame>? ScreenShareFrameReceived;

    /// <summary>Raised once when the call ends for any reason.</summary>
    event EventHandler<CallEnded>? Ended;

    /// <summary>Tells Teams the bot is recording, so participants see the recording notice (Graph <c>updateRecordingStatus</c>).</summary>
    /// <param name="recording">Whether media is being captured.</param>
    /// <param name="cancellationToken">Cancels the request.</param>
    /// <returns>A task that completes when Teams accepted the status.</returns>
    Task UpdateRecordingStatusAsync(bool recording, CancellationToken cancellationToken);

    /// <summary>Leaves the meeting.</summary>
    /// <param name="cancellationToken">Cancels the request.</param>
    /// <returns>A task that completes when the call is torn down.</returns>
    Task HangUpAsync(CancellationToken cancellationToken);
}

/// <summary>Joins meetings (the Graph communications client in TeamsBot.Media).</summary>
public interface ICallJoiner
{
    /// <summary>Joins a meeting by its Teams join URL with app-hosted media.</summary>
    /// <param name="joinUrl">The meeting's "Join" link.</param>
    /// <param name="cancellationToken">Cancels the join.</param>
    /// <returns>The joined session.</returns>
    Task<ICallSession> JoinAsync(Uri joinUrl, CancellationToken cancellationToken);
}

/// <summary>Where processed media goes (the <see cref="Bridge.WebSocketClient"/>).</summary>
public interface IBridgeSink
{
    /// <summary>Announces the meeting; see <see cref="Bridge.WebSocketClient.SetMeeting"/>.</summary>
    /// <param name="meeting">The meeting.</param>
    /// <returns><c>false</c> when rejected.</returns>
    bool SetMeeting(MeetingInfo meeting);

    /// <summary>Queues audio without blocking.</summary>
    /// <param name="chunk">The chunk.</param>
    /// <returns><c>false</c> when rejected.</returns>
    bool EnqueueAudio(PcmAudioChunk chunk);

    /// <summary>Queues a slide without blocking.</summary>
    /// <param name="frame">The JPEG.</param>
    /// <returns><c>false</c> when rejected.</returns>
    bool EnqueueVideo(EncodedVideoFrame frame);

    /// <summary>Waits for queued frames to be sent.</summary>
    /// <param name="timeout">How long to wait.</param>
    /// <returns><c>true</c> when drained.</returns>
    Task<bool> FlushAsync(TimeSpan timeout);
}
