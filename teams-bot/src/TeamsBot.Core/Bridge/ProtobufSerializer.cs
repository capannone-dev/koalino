// Serializes bot values into TeamsBridgeMessage frames defined by core/protocol/teams_bridge.proto
// (the schema shared with the Rust app, Issue #33) and enforces the Rust decoder's validation
// rules up front, so the bot never sends a frame the app would skip.

using Google.Protobuf;
using NoteTaker.TeamsBridge.V1;
using TeamsBot.Core.Model;

namespace TeamsBot.Core.Bridge;

/// <summary>Builds binary WebSocket frames for the Rust Teams bridge.</summary>
public static class ProtobufSerializer
{
    /// <summary>The only sample rate the Rust app accepts.</summary>
    public const uint SampleRateHz = 16_000;

    /// <summary>Longest audio chunk the Rust app accepts.</summary>
    public const ulong MaximumChunkDurationMs = 30_000;

    /// <summary>Largest WebSocket message the Rust server accepts (8 MiB).</summary>
    public const int MaximumMessageBytes = 8 * 1024 * 1024;

    /// <summary>The video source that becomes a slide in the Rust app.</summary>
    public const string ScreenShareSource = "screen_share";

    /// <summary>Serializes one audio chunk.</summary>
    /// <param name="chunk">Non-empty, whole 16-bit samples, at most 30 s.</param>
    /// <returns>The frame bytes.</returns>
    /// <exception cref="ArgumentException">The audio is empty, has a partial sample, or is longer than 30 s.</exception>
    public static byte[] SerializeAudio(PcmAudioChunk chunk)
    {
        ArgumentNullException.ThrowIfNull(chunk);
        if (chunk.Pcm.IsEmpty || chunk.Pcm.Length % 2 != 0)
        {
            throw new ArgumentException("Audio must contain whole 16-bit samples.", nameof(chunk));
        }
        if ((ulong)chunk.Pcm.Length > MaximumChunkDurationMs * PcmAudioChunk.BytesPerMillisecond)
        {
            throw new ArgumentException("Audio chunks must not exceed 30 s.", nameof(chunk));
        }
        var message = new TeamsBridgeMessage
        {
            Audio = new AudioChunk
            {
                Participant = ToProtocol(chunk.Participant),
                AudioData = ByteString.CopyFrom(chunk.Pcm.Span),
                TimestampMs = chunk.TimestampMs,
                SampleRate = SampleRateHz,
            },
        };
        return message.ToByteArray();
    }

    /// <summary>Serializes one screen-share frame.</summary>
    /// <param name="frame">A non-empty image that fits in one message.</param>
    /// <returns>The frame bytes, with <c>source = "screen_share"</c>.</returns>
    /// <exception cref="ArgumentException">The image is empty or too large.</exception>
    public static byte[] SerializeVideo(EncodedVideoFrame frame)
    {
        ArgumentNullException.ThrowIfNull(frame);
        if (frame.ImageData.IsEmpty)
        {
            throw new ArgumentException("Video frames must contain image data.", nameof(frame));
        }
        var message = new TeamsBridgeMessage
        {
            Video = new VideoFrame
            {
                ImageData = ByteString.CopyFrom(frame.ImageData.Span),
                TimestampMs = frame.TimestampMs,
                Source = ScreenShareSource,
            },
        };
        var bytes = message.ToByteArray();
        if (bytes.Length > MaximumMessageBytes)
        {
            throw new ArgumentException("Video frame exceeds the 8 MiB message limit.", nameof(frame));
        }
        return bytes;
    }

    /// <summary>Serializes meeting metadata, which must precede the meeting's media.</summary>
    /// <param name="meeting">A meeting with a non-blank ID.</param>
    /// <returns>The frame bytes.</returns>
    /// <exception cref="ArgumentException">The meeting ID is blank.</exception>
    public static byte[] SerializeMetadata(MeetingInfo meeting)
    {
        ArgumentNullException.ThrowIfNull(meeting);
        if (string.IsNullOrWhiteSpace(meeting.MeetingId))
        {
            throw new ArgumentException("Meeting metadata requires a meeting ID.", nameof(meeting));
        }
        var metadata = new MeetingMetadata
        {
            MeetingId = meeting.MeetingId,
            MeetingTitle = meeting.Title,
            StartTimeMs = (ulong)Math.Max(0, meeting.StartTime.ToUnixTimeMilliseconds()),
        };
        metadata.Participants.AddRange(meeting.Participants.Select(ToProtocol));
        return new TeamsBridgeMessage { Metadata = metadata }.ToByteArray();
    }

    /// <summary>Parses a frame (used by tests and diagnostics).</summary>
    /// <param name="frame">Bytes of one binary WebSocket message.</param>
    /// <returns>The envelope.</returns>
    /// <exception cref="InvalidProtocolBufferException">The bytes are not a valid message.</exception>
    public static TeamsBridgeMessage Parse(ReadOnlySpan<byte> frame) => TeamsBridgeMessage.Parser.ParseFrom(frame);

    /// <summary>Converts a participant; a missing e-mail becomes the empty string (proto3 default).</summary>
    private static ExternalParticipant ToProtocol(ParticipantInfo participant) => new()
    {
        ExternalId = participant.TeamsUserId,
        DisplayName = participant.DisplayName,
        Email = participant.Email ?? string.Empty,
    };
}
