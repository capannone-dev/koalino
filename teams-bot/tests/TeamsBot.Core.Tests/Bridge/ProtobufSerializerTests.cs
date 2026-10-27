// Specifies Protobuf serialization against the golden frames in core/protocol/fixtures, which
// `protoc` encoded from the shared teams_bridge.proto and which the Rust decoder is tested against.
// Byte-for-byte equality here therefore proves the bot's frames decode in the Rust app.

using NoteTaker.TeamsBridge.V1;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Model;
using Xunit;

namespace TeamsBot.Core.Tests.Bridge;

/// <summary>Golden-frame and validation tests for <see cref="ProtobufSerializer"/>.</summary>
public sealed class ProtobufSerializerTests
{
    /// <summary>Reads one golden frame copied next to the test assembly.</summary>
    private static byte[] Fixture(string name) =>
        File.ReadAllBytes(Path.Combine(AppContext.BaseDirectory, "Fixtures", name));

    /// <summary>An audio chunk serializes to exactly the golden audio frame.</summary>
    [Fact]
    public void AudioChunkMatchesTheGoldenFrame()
    {
        var chunk = new PcmAudioChunk(
            new ParticipantInfo("teams-user-1", "Ada Lovelace", "ada@example.com"),
            new byte[] { 1, 0, 2, 0, 0xFF, 0xFF },
            TimestampMs: 12_000);

        Assert.Equal(Fixture("audio.bin"), ProtobufSerializer.SerializeAudio(chunk));
    }

    /// <summary>A screen-share frame serializes to exactly the golden video frame.</summary>
    [Fact]
    public void VideoFrameMatchesTheGoldenFrame()
    {
        var frame = new EncodedVideoFrame(new byte[] { 0x89, (byte)'P', (byte)'N', (byte)'G' }, TimestampMs: 34_000);

        Assert.Equal(Fixture("video.bin"), ProtobufSerializer.SerializeVideo(frame));
    }

    /// <summary>Meeting metadata serializes to exactly the golden metadata frame.</summary>
    [Fact]
    public void MeetingMetadataMatchesTheGoldenFrame()
    {
        var meeting = new MeetingInfo(
            "19:meeting_abc@thread.v2",
            "Weekly sync",
            DateTimeOffset.FromUnixTimeMilliseconds(1_793_000_000_000),
            new[]
            {
                new ParticipantInfo("teams-user-1", "Ada Lovelace"),
                new ParticipantInfo("teams-user-2", "Alan Turing", "alan@example.com"),
            });

        Assert.Equal(Fixture("metadata.bin"), ProtobufSerializer.SerializeMetadata(meeting));
    }

    /// <summary>Golden frames parse back into the expected envelope cases.</summary>
    [Fact]
    public void GoldenFramesParseBack()
    {
        var audio = ProtobufSerializer.Parse(Fixture("audio.bin"));
        Assert.Equal(TeamsBridgeMessage.PayloadOneofCase.Audio, audio.PayloadCase);
        Assert.Equal(16_000u, audio.Audio.SampleRate);
        Assert.Equal("Ada Lovelace", audio.Audio.Participant.DisplayName);

        var video = ProtobufSerializer.Parse(Fixture("video.bin"));
        Assert.Equal("screen_share", video.Video.Source);

        var metadata = ProtobufSerializer.Parse(Fixture("metadata.bin"));
        Assert.Equal(2, metadata.Metadata.Participants.Count);
        Assert.Equal(string.Empty, metadata.Metadata.Participants[0].Email);
    }

    /// <summary>Audio the Rust decoder would reject is refused before it is sent.</summary>
    [Theory]
    [InlineData(0)]
    [InlineData(3)]
    [InlineData(30_000 * 32 + 2)]
    public void InvalidAudioIsRejected(int byteCount)
    {
        var chunk = new PcmAudioChunk(new ParticipantInfo("teams-user-1", "Ada"), new byte[byteCount], 0);

        Assert.Throws<ArgumentException>(() => ProtobufSerializer.SerializeAudio(chunk));
    }

    /// <summary>A 30 s chunk is the longest accepted, and its duration is derived from its size.</summary>
    [Fact]
    public void ThirtySecondChunkIsAccepted()
    {
        var chunk = new PcmAudioChunk(new ParticipantInfo("teams-user-1", "Ada"), new byte[30_000 * 32], 0);

        Assert.Equal(30_000u, chunk.DurationMs);
        Assert.NotEmpty(ProtobufSerializer.SerializeAudio(chunk));
    }

    /// <summary>Empty or oversized images are refused (the Rust server caps messages at 8 MiB).</summary>
    [Fact]
    public void InvalidVideoIsRejected()
    {
        Assert.Throws<ArgumentException>(() => ProtobufSerializer.SerializeVideo(new EncodedVideoFrame(Array.Empty<byte>(), 0)));
        Assert.Throws<ArgumentException>(() =>
            ProtobufSerializer.SerializeVideo(new EncodedVideoFrame(new byte[ProtobufSerializer.MaximumMessageBytes], 0)));
    }

    /// <summary>Metadata without a meeting ID is refused (the Rust decoder requires one).</summary>
    [Fact]
    public void MetadataWithoutMeetingIdIsRejected()
    {
        var meeting = new MeetingInfo(" ", "Title", DateTimeOffset.UnixEpoch, Array.Empty<ParticipantInfo>());

        Assert.Throws<ArgumentException>(() => ProtobufSerializer.SerializeMetadata(meeting));
    }
}
