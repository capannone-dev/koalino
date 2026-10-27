// Specifies how screen-share frames (NV12, as delivered by the Teams video-based screen sharing
// socket) become throttled, de-duplicated JPEG frames that fit in one bridge message.

using Microsoft.Extensions.Logging;
using SkiaSharp;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Tests.Support;
using TeamsBot.Core.Video;
using Xunit;

namespace TeamsBot.Core.Tests.Video;

/// <summary>Tests for <see cref="VideoProcessor"/>.</summary>
public sealed class VideoProcessorTests
{
    private readonly RecordingLogger<VideoProcessor> logger = new();

    /// <summary>Builds a processor with the given options.</summary>
    private VideoProcessor Processor(double fps = 1, int quality = 80, bool skipUnchanged = true, int maximumBytes = 8_000_000) =>
        new(new VideoProcessingOptions { Fps = fps, JpegQuality = quality, SkipUnchangedFrames = skipUnchanged, MaximumEncodedBytes = maximumBytes }, logger);

    /// <summary>Builds a solid-colour NV12 frame; padding bytes beyond the width hold garbage.</summary>
    private static RawVideoFrame SolidFrame(int width, int height, byte luma, byte chromaU, byte chromaV, ulong offsetMs, int? stride = null)
    {
        var rowStride = stride ?? width;
        var data = new byte[rowStride * height * 3 / 2];
        Array.Fill(data, (byte)0xEE);
        for (var row = 0; row < height; row++)
        {
            data.AsSpan(row * rowStride, width).Fill(luma);
        }
        for (var row = 0; row < height / 2; row++)
        {
            var chromaRow = data.AsSpan((rowStride * height) + (row * rowStride), width);
            for (var column = 0; column < width; column += 2)
            {
                chromaRow[column] = chromaU;
                chromaRow[column + 1] = chromaV;
            }
        }
        return new RawVideoFrame(data, width, height, rowStride, offsetMs);
    }

    /// <summary>Builds a frame of pseudo-random luma (hard to compress).</summary>
    private static RawVideoFrame NoiseFrame(int width, int height, ulong offsetMs, int seed)
    {
        var data = new byte[width * height * 3 / 2];
        new Random(seed).NextBytes(data);
        return new RawVideoFrame(data, width, height, width, offsetMs);
    }

    /// <summary>Decodes an emitted JPEG, failing the test when it is not one.</summary>
    private static SKBitmap Decode(ReadOnlyMemory<byte> jpeg)
    {
        Assert.Equal(new byte[] { 0xFF, 0xD8, 0xFF }, jpeg.Span[..3].ToArray());
        return SKBitmap.Decode(jpeg.ToArray()) ?? throw new InvalidOperationException("not a decodable image");
    }

    /// <summary>NV12 is converted (BT.601) and encoded as a JPEG with the frame's size and colour.</summary>
    [Theory]
    [InlineData(81, 90, 240, 255, 0, 0)]
    [InlineData(145, 54, 34, 0, 255, 0)]
    [InlineData(126, 128, 128, 128, 128, 128)]
    public void Nv12IsEncodedAsJpeg(byte luma, byte chromaU, byte chromaV, byte red, byte green, byte blue)
    {
        var encoded = Processor().ProcessFrame(SolidFrame(64, 32, luma, chromaU, chromaV, offsetMs: 7_000));

        Assert.NotNull(encoded);
        Assert.Equal(7_000u, encoded.TimestampMs);
        using var bitmap = Decode(encoded.ImageData);
        Assert.Equal((64, 32), (bitmap.Width, bitmap.Height));
        var pixel = bitmap.GetPixel(32, 16);
        Assert.InRange(pixel.Red, red - 12, red + 12);
        Assert.InRange(pixel.Green, green - 12, green + 12);
        Assert.InRange(pixel.Blue, blue - 12, blue + 12);
    }

    /// <summary>Row padding (stride wider than the image) is ignored.</summary>
    [Fact]
    public void RowPaddingIsIgnored()
    {
        var encoded = Processor().ProcessFrame(SolidFrame(64, 32, 81, 90, 240, 0, stride: 80));

        Assert.NotNull(encoded);
        using var bitmap = Decode(encoded.ImageData);
        Assert.Equal(64, bitmap.Width);
        Assert.InRange(bitmap.GetPixel(63, 31).Red, 240, 255);
    }

    /// <summary>At 1 FPS, a 15 FPS stream yields one frame per second.</summary>
    [Fact]
    public void FramesAreThrottledToTheConfiguredRate()
    {
        var processor = Processor(fps: 1, skipUnchanged: false);
        var emittedOffsets = new List<ulong>();
        for (ulong offsetMs = 0; offsetMs < 3_000; offsetMs += 66)
        {
            var encoded = processor.ProcessFrame(SolidFrame(32, 16, 128, 128, 128, offsetMs));
            if (encoded is not null)
            {
                emittedOffsets.Add(encoded.TimestampMs);
            }
        }

        Assert.Equal(3, emittedOffsets.Count);
        Assert.Equal(0u, emittedOffsets[0]);
        Assert.All(emittedOffsets.Zip(emittedOffsets.Skip(1)), pair => Assert.True(pair.Second - pair.First >= 1_000));
    }

    /// <summary>A static slide is sent once; the next change is sent again.</summary>
    [Fact]
    public void UnchangedFramesAreSkipped()
    {
        var processor = Processor();

        Assert.NotNull(processor.ProcessFrame(SolidFrame(64, 32, 81, 90, 240, 0)));
        Assert.Null(processor.ProcessFrame(SolidFrame(64, 32, 81, 90, 240, 1_000)));
        Assert.Null(processor.ProcessFrame(SolidFrame(64, 32, 81, 90, 240, 2_000)));
        Assert.NotNull(processor.ProcessFrame(SolidFrame(64, 32, 145, 54, 34, 3_000)));
    }

    /// <summary>A resolution change mid-stream is handled and always counts as a change.</summary>
    [Fact]
    public void ResolutionChangesAreHandled()
    {
        var processor = Processor();
        Assert.NotNull(processor.ProcessFrame(SolidFrame(64, 32, 81, 90, 240, 0)));

        var resized = processor.ProcessFrame(SolidFrame(128, 72, 81, 90, 240, 1_000));

        Assert.NotNull(resized);
        using var bitmap = Decode(resized.ImageData);
        Assert.Equal((128, 72), (bitmap.Width, bitmap.Height));
    }

    /// <summary>A frame that encodes too large is re-encoded smaller until it fits.</summary>
    [Fact]
    public void OversizedFramesAreShrunkToFit()
    {
        var encoded = Processor(maximumBytes: 20_000).ProcessFrame(NoiseFrame(640, 360, 0, seed: 1));

        Assert.NotNull(encoded);
        Assert.InRange(encoded.ImageData.Length, 1, 20_000);
        using var bitmap = Decode(encoded.ImageData);
        Assert.True(bitmap.Width <= 640);
    }

    /// <summary>A frame that cannot fit at all is dropped with a warning.</summary>
    [Fact]
    public void FramesThatCannotFitAreDropped()
    {
        Assert.Null(Processor(maximumBytes: 50).ProcessFrame(NoiseFrame(640, 360, 0, seed: 2)));
        Assert.True(logger.Contains(LogLevel.Warning, "dropped"));
    }

    /// <summary>Malformed frames are skipped with a warning and never throw.</summary>
    [Fact]
    public void MalformedFramesAreSkipped()
    {
        var processor = Processor();

        Assert.Null(processor.ProcessFrame(new RawVideoFrame(new byte[10], 64, 32, 64, 0)));
        Assert.Null(processor.ProcessFrame(new RawVideoFrame(new byte[64 * 33 * 3 / 2], 64, 33, 64, 0)));
        Assert.Null(processor.ProcessFrame(new RawVideoFrame(new byte[100], 0, 0, 0, 0)));
        Assert.Null(processor.ProcessFrame(new RawVideoFrame(new byte[64 * 32 * 3 / 2], 64, 32, 32, 0)));
        Assert.True(logger.Contains(LogLevel.Warning, "skipped"));
    }

    /// <summary>After a reset (screen share restarted), the next frame is sent even if unchanged.</summary>
    [Fact]
    public void ResetForgetsThePreviousFrame()
    {
        var processor = Processor();
        Assert.NotNull(processor.ProcessFrame(SolidFrame(64, 32, 81, 90, 240, 0)));

        processor.Reset();

        Assert.NotNull(processor.ProcessFrame(SolidFrame(64, 32, 81, 90, 240, 100)));
    }

    /// <summary>Out-of-range options are rejected.</summary>
    [Theory]
    [InlineData(0, 80)]
    [InlineData(31, 80)]
    [InlineData(1, 0)]
    [InlineData(1, 101)]
    public void InvalidOptionsAreRejected(double fps, int quality)
    {
        Assert.Throws<ArgumentException>(() => Processor(fps, quality));
    }
}
