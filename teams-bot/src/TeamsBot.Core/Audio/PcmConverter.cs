// Converts 16-bit PCM in the formats the Teams media platform can deliver into the 16 kHz mono
// PCM the Rust app requires. Teams normally delivers 16 kHz mono already (passthrough); other
// rates are downsampled with a centred box filter, which suppresses aliasing well enough for
// speech transcription.

using System.Buffers.Binary;
using TeamsBot.Core.Abstractions;

namespace TeamsBot.Core.Audio;

/// <summary>Stateless PCM format conversion.</summary>
public static class PcmConverter
{
    /// <summary>The output sample rate.</summary>
    public const int TargetSampleRateHz = 16_000;

    /// <summary>The most interleaved channels accepted.</summary>
    public const int MaximumChannels = 8;

    /// <summary>Converts PCM to 16 kHz mono.</summary>
    /// <param name="pcm">Signed 16-bit little-endian interleaved PCM with whole frames.</param>
    /// <param name="format">Its layout; the rate must be at least 16 kHz.</param>
    /// <returns>16 kHz mono PCM (the input array is returned unchanged when it already matches).</returns>
    /// <exception cref="ArgumentException">The format is unsupported or the data has partial frames.</exception>
    public static byte[] ToPcm16kMono(ReadOnlyMemory<byte> pcm, AudioFormatInfo format)
    {
        if (format.Channels is < 1 or > MaximumChannels)
        {
            throw new ArgumentException($"Unsupported channel count {format.Channels}.", nameof(format));
        }
        if (format.SampleRateHz < TargetSampleRateHz)
        {
            throw new ArgumentException($"Unsupported sample rate {format.SampleRateHz} Hz.", nameof(format));
        }
        var bytesPerFrame = 2 * format.Channels;
        if (pcm.Length % bytesPerFrame != 0)
        {
            throw new ArgumentException("PCM data contains a partial sample frame.", nameof(pcm));
        }
        if (format is { Channels: 1, SampleRateHz: TargetSampleRateHz })
        {
            return pcm.ToArray();
        }
        var mono = Downmix(pcm.Span, format.Channels);
        var resampled = format.SampleRateHz == TargetSampleRateHz ? mono : Downsample(mono, format.SampleRateHz);
        var output = new byte[resampled.Length * 2];
        for (var index = 0; index < resampled.Length; index++)
        {
            BinaryPrimitives.WriteInt16LittleEndian(output.AsSpan(index * 2), resampled[index]);
        }
        return output;
    }

    /// <summary>Averages interleaved channels into one.</summary>
    private static short[] Downmix(ReadOnlySpan<byte> pcm, int channels)
    {
        var frameCount = pcm.Length / (2 * channels);
        var mono = new short[frameCount];
        for (var frame = 0; frame < frameCount; frame++)
        {
            var sum = 0;
            for (var channel = 0; channel < channels; channel++)
            {
                sum += BinaryPrimitives.ReadInt16LittleEndian(pcm.Slice(((frame * channels) + channel) * 2, 2));
            }
            mono[frame] = (short)(sum / channels);
        }
        return mono;
    }

    /// <summary>Downsamples to 16 kHz by averaging the source samples around each output instant.</summary>
    private static short[] Downsample(short[] source, int sourceRateHz)
    {
        var ratio = (double)sourceRateHz / TargetSampleRateHz;
        var halfWidth = ratio / 2;
        var outputCount = (int)((long)source.Length * TargetSampleRateHz / sourceRateHz);
        var output = new short[outputCount];
        for (var index = 0; index < outputCount; index++)
        {
            var centre = index * ratio;
            var first = Math.Max(0, (int)Math.Ceiling(centre - halfWidth + 1e-9));
            var last = Math.Min(source.Length - 1, (int)Math.Floor(centre + halfWidth - 1e-9));
            long sum = 0;
            for (var sourceIndex = first; sourceIndex <= last; sourceIndex++)
            {
                sum += source[sourceIndex];
            }
            output[index] = (short)(sum / Math.Max(1, last - first + 1));
        }
        return output;
    }
}
