// Portable values the bot exchanges between its media processing and the Rust bridge. They
// carry no Teams SDK types, so processing and serialization are testable on any OS.

namespace TeamsBot.Core.Model;

/// <summary>A meeting participant as reported by the Teams roster.</summary>
/// <param name="TeamsUserId">Stable Teams user ID (Azure AD object ID, or the participant ID for guests); the identity-mapping key in the Rust app.</param>
/// <param name="DisplayName">Name shown in Teams; used as the initial local identity name.</param>
/// <param name="Email">Optional e-mail address; <c>null</c> when unknown. Never logged.</param>
public sealed record ParticipantInfo(string TeamsUserId, string DisplayName, string? Email = null);

/// <summary>A meeting the bot joined.</summary>
/// <param name="MeetingId">Teams meeting (thread) ID; the Rust app derives its local meeting ID from it.</param>
/// <param name="Title">Meeting subject; may be empty. Never logged.</param>
/// <param name="StartTime">When the bot joined; media timestamps are relative to it.</param>
/// <param name="Participants">Participants known when the metadata is sent.</param>
public sealed record MeetingInfo(
    string MeetingId,
    string Title,
    DateTimeOffset StartTime,
    IReadOnlyList<ParticipantInfo> Participants);

/// <summary>One participant's isolated audio, as 16 kHz mono signed 16-bit little-endian PCM.</summary>
/// <param name="Participant">Who spoke.</param>
/// <param name="Pcm">The samples; never logged.</param>
/// <param name="TimestampMs">Chunk start in milliseconds since <see cref="MeetingInfo.StartTime"/>.</param>
public sealed record PcmAudioChunk(ParticipantInfo Participant, ReadOnlyMemory<byte> Pcm, ulong TimestampMs)
{
    /// <summary>Bytes of 16 kHz mono 16-bit PCM per millisecond.</summary>
    public const int BytesPerMillisecond = 32;

    /// <summary>The chunk length in milliseconds (rounded down).</summary>
    public ulong DurationMs => (ulong)(Pcm.Length / BytesPerMillisecond);
}

/// <summary>An encoded screen-share frame (JPEG).</summary>
/// <param name="ImageData">The encoded image; never logged.</param>
/// <param name="TimestampMs">Capture time in milliseconds since <see cref="MeetingInfo.StartTime"/>.</param>
public sealed record EncodedVideoFrame(ReadOnlyMemory<byte> ImageData, ulong TimestampMs);
