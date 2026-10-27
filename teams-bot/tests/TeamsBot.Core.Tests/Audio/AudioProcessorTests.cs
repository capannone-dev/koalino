// Specifies how unmixed Teams audio frames become per-participant 16 kHz mono PCM chunks:
// format conversion, speaker tagging through the roster, and chunk boundaries.

using Microsoft.Extensions.Logging;
using NSubstitute;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Audio;
using TeamsBot.Core.Model;
using TeamsBot.Core.Tests.Support;
using Xunit;

namespace TeamsBot.Core.Tests.Audio;

/// <summary>Tests for <see cref="AudioProcessor"/> and <see cref="PcmConverter"/>.</summary>
public sealed class AudioProcessorTests
{
    private static readonly ParticipantInfo Ada = new("aad-ada", "Ada Lovelace");
    private static readonly ParticipantInfo Alan = new("aad-alan", "Alan Turing");
    private static readonly AudioFormatInfo Pcm16kMono = new(16_000, 1);

    private readonly IParticipantDirectory directory = Substitute.For<IParticipantDirectory>();
    private readonly RecordingLogger<AudioProcessor> logger = new();

    /// <summary>Maps media source 1 to Ada and 2 to Alan.</summary>
    public AudioProcessorTests()
    {
        directory.TryResolve(1, out Arg.Any<ParticipantInfo?>())
            .Returns(call => { call[1] = Ada; return true; });
        directory.TryResolve(2, out Arg.Any<ParticipantInfo?>())
            .Returns(call => { call[1] = Alan; return true; });
    }

    /// <summary>Builds a processor with the given limits.</summary>
    private AudioProcessor Processor(int maximumChunkMs = 10_000, int silenceGapMs = 600) =>
        new(new AudioChunkingOptions { MaximumChunkDurationMs = maximumChunkMs, SilenceGapMs = silenceGapMs }, directory, logger);

    /// <summary>Builds a 20 ms 16 kHz mono frame whose samples all equal <paramref name="value"/>.</summary>
    private static UnmixedAudioFrame Frame(uint sourceId, ulong offsetMs, short value = 100) =>
        new(sourceId, Samples(Enumerable.Repeat(value, 320)), Pcm16kMono, offsetMs);

    /// <summary>Encodes samples as little-endian 16-bit PCM.</summary>
    private static byte[] Samples(IEnumerable<short> samples) =>
        samples.SelectMany(sample => BitConverter.GetBytes(sample)).ToArray();

    /// <summary>Decodes little-endian 16-bit PCM.</summary>
    private static short[] Decode(ReadOnlyMemory<byte> pcm)
    {
        var samples = new short[pcm.Length / 2];
        for (var index = 0; index < samples.Length; index++)
        {
            samples[index] = BitConverter.ToInt16(pcm.Span.Slice(index * 2, 2));
        }
        return samples;
    }

    /// <summary>16 kHz mono audio passes through unchanged.</summary>
    [Fact]
    public void SixteenKilohertzMonoPassesThrough()
    {
        var samples = Samples(Enumerable.Range(0, 320).Select(index => (short)index));

        Assert.Equal(samples, PcmConverter.ToPcm16kMono(samples, Pcm16kMono));
    }

    /// <summary>Stereo is downmixed by averaging channels.</summary>
    [Fact]
    public void StereoIsDownmixed()
    {
        var stereo = Samples(new short[] { 1000, 3000, -2000, -4000 });

        Assert.Equal(new short[] { 2000, -3000 }, Decode(PcmConverter.ToPcm16kMono(stereo, new AudioFormatInfo(16_000, 2))));
    }

    /// <summary>48 kHz and 44.1 kHz audio is resampled to 16 kHz without distorting a tone.</summary>
    [Theory]
    [InlineData(48_000)]
    [InlineData(44_100)]
    [InlineData(24_000)]
    public void HigherRatesAreResampledTo16Kilohertz(int sourceRateHz)
    {
        const double ToneHz = 440;
        var sourceSampleCount = sourceRateHz / 10;
        var tone = Enumerable.Range(0, sourceSampleCount)
            .Select(index => (short)(10_000 * Math.Sin(2 * Math.PI * ToneHz * index / sourceRateHz)));

        var resampled = Decode(PcmConverter.ToPcm16kMono(Samples(tone), new AudioFormatInfo(sourceRateHz, 1)));

        Assert.Equal(1_600, resampled.Length);
        for (var index = 10; index < resampled.Length - 10; index++)
        {
            var expected = 10_000 * Math.Sin(2 * Math.PI * ToneHz * index / 16_000);
            Assert.InRange(resampled[index], expected - 600, expected + 600);
        }
    }

    /// <summary>Unsupported formats are rejected by the converter.</summary>
    [Theory]
    [InlineData(8_000, 1)]
    [InlineData(16_000, 0)]
    [InlineData(0, 1)]
    public void UnsupportedFormatsAreRejected(int sampleRateHz, int channels)
    {
        Assert.Throws<ArgumentException>(() => PcmConverter.ToPcm16kMono(new byte[4], new AudioFormatInfo(sampleRateHz, channels)));
    }

    /// <summary>Contiguous frames of one speaker form one chunk tagged with that participant.</summary>
    [Fact]
    public void ContiguousFramesFormOneTaggedChunk()
    {
        var processor = Processor();
        for (ulong frameIndex = 0; frameIndex < 50; frameIndex++)
        {
            Assert.Empty(processor.ProcessFrame(Frame(1, 5_000 + (frameIndex * 20))));
        }

        var chunk = Assert.Single(processor.FlushAll());
        Assert.Equal(Ada, chunk.Participant);
        Assert.Equal(5_000u, chunk.TimestampMs);
        Assert.Equal(1_000u, chunk.DurationMs);
    }

    /// <summary>A chunk is cut when it reaches the maximum duration, and the next one continues the timeline.</summary>
    [Fact]
    public void ChunksAreCutAtTheMaximumDuration()
    {
        var processor = Processor(maximumChunkMs: 1_000);
        var emitted = new List<PcmAudioChunk>();
        for (ulong frameIndex = 0; frameIndex < 75; frameIndex++)
        {
            emitted.AddRange(processor.ProcessFrame(Frame(1, frameIndex * 20)));
        }
        emitted.AddRange(processor.FlushAll());

        Assert.Equal(new ulong[] { 0, 1_000 }, emitted.Select(chunk => chunk.TimestampMs));
        Assert.Equal(new ulong[] { 1_000, 500 }, emitted.Select(chunk => chunk.DurationMs));
    }

    /// <summary>A silence gap longer than the threshold ends the chunk; the next frame starts a new one.</summary>
    [Fact]
    public void SilenceGapEndsTheChunk()
    {
        var processor = Processor();
        processor.ProcessFrame(Frame(1, 0));
        processor.ProcessFrame(Frame(1, 20));

        var ended = Assert.Single(processor.ProcessFrame(Frame(1, 2_000)));
        Assert.Equal(0u, ended.TimestampMs);
        Assert.Equal(40u, ended.DurationMs);
        Assert.Equal(2_000u, Assert.Single(processor.FlushAll()).TimestampMs);
    }

    /// <summary>Short gaps are filled with silence so the chunk keeps real-time length.</summary>
    [Fact]
    public void ShortGapsArePaddedWithSilence()
    {
        var processor = Processor();
        processor.ProcessFrame(Frame(1, 0, value: 500));
        processor.ProcessFrame(Frame(1, 100, value: 500));

        var chunk = Assert.Single(processor.FlushAll());
        Assert.Equal(120u, chunk.DurationMs);
        var samples = Decode(chunk.Pcm);
        Assert.Equal(500, samples[0]);
        Assert.Equal(0, samples[400]);
        Assert.Equal(500, samples[^1]);
    }

    /// <summary>Speakers are chunked independently and idle speakers are flushed by time.</summary>
    [Fact]
    public void SpeakersAreChunkedIndependently()
    {
        var processor = Processor();
        processor.ProcessFrame(Frame(1, 0));
        processor.ProcessFrame(Frame(2, 0));
        var alanFirstChunk = Assert.Single(processor.ProcessFrame(Frame(2, 1_000)));
        Assert.Equal(Alan, alanFirstChunk.Participant);

        var adaChunk = Assert.Single(processor.FlushIdle(nowOffsetMs: 1_000));
        Assert.Equal(Ada, adaChunk.Participant);
        var alanSecondChunk = Assert.Single(processor.FlushAll());
        Assert.Equal(Alan, alanSecondChunk.Participant);
        Assert.Equal(1_000u, alanSecondChunk.TimestampMs);
    }

    /// <summary>An unresolved media source falls back to a stable placeholder participant and logs a warning.</summary>
    [Fact]
    public void UnknownSourcesUseAPlaceholderParticipant()
    {
        var processor = Processor();
        processor.ProcessFrame(Frame(77, 0));

        var chunk = Assert.Single(processor.FlushAll());
        Assert.Equal("teams-media-source-77", chunk.Participant.TeamsUserId);
        Assert.True(logger.Contains(LogLevel.Warning, "77"));
    }

    /// <summary>Malformed frames are skipped with a warning and never throw.</summary>
    [Fact]
    public void MalformedFramesAreSkipped()
    {
        var processor = Processor();

        Assert.Empty(processor.ProcessFrame(new UnmixedAudioFrame(1, new byte[3], Pcm16kMono, 0)));
        Assert.Empty(processor.ProcessFrame(new UnmixedAudioFrame(1, new byte[4], new AudioFormatInfo(8_000, 1), 0)));
        Assert.Empty(processor.FlushAll());
        Assert.True(logger.Contains(LogLevel.Warning, "skipped"));
    }

    /// <summary>Chunk limits outside what the Rust app accepts are rejected.</summary>
    [Theory]
    [InlineData(30_001, 600)]
    [InlineData(0, 600)]
    [InlineData(10_000, 0)]
    public void InvalidOptionsAreRejected(int maximumChunkMs, int silenceGapMs)
    {
        Assert.Throws<ArgumentException>(() => Processor(maximumChunkMs, silenceGapMs));
    }
}
