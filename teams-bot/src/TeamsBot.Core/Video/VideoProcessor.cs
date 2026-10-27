// Turns screen-share frames into JPEG slides for the Rust app.
//
// Frames are sampled at most every 1/Fps seconds (the Teams socket delivers far more). A sampled
// frame identical to the last sent one (compared by a coarse luma/chroma grid) is skipped, so a
// static slide is sent once. NV12 is converted to RGB (BT.601, limited range) and encoded with
// SkiaSharp at JpegQuality; if the result exceeds MaximumEncodedBytes (the Rust server accepts
// 8 MiB messages), quality is lowered and then the image is halved until it fits.

using System.Diagnostics.CodeAnalysis;
using Microsoft.Extensions.Logging;
using SkiaSharp;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Model;

namespace TeamsBot.Core.Video;

/// <summary>Screen-share capture settings (configuration <c>TeamsBot:VideoFps</c>, <c>TeamsBot:VideoQuality</c>).</summary>
public sealed class VideoProcessingOptions
{
    /// <summary>The highest supported sampling rate.</summary>
    public const double MaximumFps = 30;

    /// <summary>Frames sampled per second (0 &lt; Fps ≤ <see cref="MaximumFps"/>; default 1).</summary>
    public double Fps { get; set; } = 1;

    /// <summary>JPEG quality, 1–100 (default 80).</summary>
    public int JpegQuality { get; set; } = 80;

    /// <summary>Whether frames identical to the last sent one are skipped (default true).</summary>
    public bool SkipUnchangedFrames { get; set; } = true;

    /// <summary>Largest encoded image; leaves room for the envelope within the 8 MiB message limit.</summary>
    public int MaximumEncodedBytes { get; set; } = ProtobufSerializer.MaximumMessageBytes - 1024;

    /// <summary>Rejects out-of-range values.</summary>
    /// <exception cref="ArgumentException">A value is out of range.</exception>
    public void Validate()
    {
        if (!(Fps > 0 && Fps <= MaximumFps))
        {
            throw new ArgumentException("Fps must be greater than 0 and at most 30.", nameof(Fps));
        }
        if (JpegQuality is < 1 or > 100)
        {
            throw new ArgumentException("JpegQuality must be between 1 and 100.", nameof(JpegQuality));
        }
        if (MaximumEncodedBytes < 1 || MaximumEncodedBytes > ProtobufSerializer.MaximumMessageBytes - 64)
        {
            throw new ArgumentException("MaximumEncodedBytes must fit in one bridge message.", nameof(MaximumEncodedBytes));
        }
    }
}

/// <summary>Throttles, de-duplicates, and JPEG-encodes screen-share frames. Thread-safe.</summary>
public sealed partial class VideoProcessor
{
    /// <summary>Grid columns and rows sampled for change detection.</summary>
    private const int FingerprintColumns = 32;
    private const int FingerprintRows = 18;

    /// <summary>Mean absolute difference (0–255) above which a frame counts as changed.</summary>
    private const double ChangeThreshold = 1.5;

    /// <summary>Qualities tried, after the configured one, when an image is too large.</summary>
    private static readonly int[] FallbackQualities = { 60, 40, 25 };

    /// <summary>Smallest width an oversized image is shrunk to before it is dropped.</summary>
    private const int MinimumShrunkWidth = 64;

    private readonly VideoProcessingOptions options;
    private readonly ILogger<VideoProcessor> logger;
    private readonly ulong samplingIntervalMs;
    private readonly object gate = new();
    private ulong? lastSampledOffsetMs;
    private Fingerprint? lastSentFingerprint;

    /// <summary>Creates a processor.</summary>
    /// <param name="options">Capture settings; validated here.</param>
    /// <param name="logger">Receives warnings (never image data).</param>
    /// <exception cref="ArgumentException">The options are invalid.</exception>
    public VideoProcessor(VideoProcessingOptions options, ILogger<VideoProcessor> logger)
    {
        ArgumentNullException.ThrowIfNull(options);
        options.Validate();
        this.options = options;
        this.logger = logger ?? throw new ArgumentNullException(nameof(logger));
        samplingIntervalMs = (ulong)Math.Ceiling(1_000 / options.Fps);
    }

    /// <summary>Forgets the last frame (call when a screen share starts or stops).</summary>
    public void Reset()
    {
        lock (gate)
        {
            lastSampledOffsetMs = null;
            lastSentFingerprint = null;
        }
    }

    /// <summary>Processes one frame.</summary>
    /// <param name="frame">An NV12 frame; malformed frames are skipped with a warning.</param>
    /// <returns>The JPEG to send, or <c>null</c> when the frame is throttled, unchanged, malformed, or cannot fit.</returns>
    public EncodedVideoFrame? ProcessFrame(RawVideoFrame frame)
    {
        ArgumentNullException.ThrowIfNull(frame);
        if (!IsWellFormed(frame, out var problem))
        {
            LogFrameSkipped(problem);
            return null;
        }
        Fingerprint fingerprint;
        lock (gate)
        {
            if (lastSampledOffsetMs is { } lastOffset && frame.OffsetMs < lastOffset + samplingIntervalMs)
            {
                return null;
            }
            lastSampledOffsetMs = frame.OffsetMs;
            fingerprint = Fingerprint.Of(frame);
            if (options.SkipUnchangedFrames && lastSentFingerprint is { } previous && !fingerprint.DiffersFrom(previous))
            {
                return null;
            }
        }

        try
        {
            var jpeg = Encode(frame);
            if (jpeg is null)
            {
                LogFrameTooLarge(frame.Width, frame.Height, options.MaximumEncodedBytes);
                return null;
            }
            lock (gate)
            {
                if (lastSentFingerprint is { } previous && (previous.Width, previous.Height) != (frame.Width, frame.Height))
                {
                    LogResolutionChanged(previous.Width, previous.Height, frame.Width, frame.Height);
                }
                lastSentFingerprint = fingerprint;
            }
            return new EncodedVideoFrame(jpeg, frame.OffsetMs);
        }
        catch (Exception exception) when (exception is InvalidOperationException or ArgumentException)
        {
            LogEncodingFailed(exception);
            return null;
        }
    }

    /// <summary>Checks the NV12 layout against the buffer.</summary>
    private static bool IsWellFormed(RawVideoFrame frame, [NotNullWhen(false)] out string? problem)
    {
        problem = frame switch
        {
            { Width: <= 0 } or { Height: <= 0 } => "non-positive size",
            _ when frame.Width % 2 != 0 || frame.Height % 2 != 0 => "odd size",
            _ when frame.Stride < frame.Width => "stride narrower than the image",
            _ when (long)frame.Stride * frame.Height * 3 / 2 > frame.Data.Length => "buffer shorter than the frame",
            _ => null,
        };
        return problem is null;
    }

    /// <summary>Encodes the frame, shrinking it until it fits; <c>null</c> when it cannot fit.</summary>
    private byte[]? Encode(RawVideoFrame frame)
    {
        using var bitmap = ToBitmap(frame);
        var qualities = new[] { options.JpegQuality }.Concat(FallbackQualities.Where(quality => quality < options.JpegQuality)).ToArray();
        var current = bitmap;
        try
        {
            while (true)
            {
                foreach (var quality in qualities)
                {
                    using var data = current.Encode(SKEncodedImageFormat.Jpeg, quality)
                        ?? throw new InvalidOperationException("JPEG encoding failed.");
                    if (data.Size <= options.MaximumEncodedBytes)
                    {
                        return data.ToArray();
                    }
                }
                if (current.Width / 2 < MinimumShrunkWidth)
                {
                    return null;
                }
                var shrunk = current.Resize(
                    new SKImageInfo(current.Width / 2, Math.Max(1, current.Height / 2), SKColorType.Rgba8888, SKAlphaType.Opaque),
                    new SKSamplingOptions(SKFilterMode.Linear))
                    ?? throw new InvalidOperationException("Image resize failed.");
                if (!ReferenceEquals(current, bitmap))
                {
                    current.Dispose();
                }
                current = shrunk;
            }
        }
        finally
        {
            if (!ReferenceEquals(current, bitmap))
            {
                current.Dispose();
            }
        }
    }

    /// <summary>Converts NV12 (BT.601 limited range) to an opaque RGBA bitmap.</summary>
    private static SKBitmap ToBitmap(RawVideoFrame frame)
    {
        var bitmap = new SKBitmap(new SKImageInfo(frame.Width, frame.Height, SKColorType.Rgba8888, SKAlphaType.Opaque));
        var pixels = bitmap.GetPixelSpan();
        var source = frame.Data.Span;
        var chromaPlaneOffset = frame.Stride * frame.Height;
        for (var row = 0; row < frame.Height; row++)
        {
            var lumaRow = source.Slice(row * frame.Stride, frame.Width);
            var chromaRow = source.Slice(chromaPlaneOffset + ((row / 2) * frame.Stride), frame.Width);
            var pixelRow = pixels.Slice(row * bitmap.RowBytes, frame.Width * 4);
            for (var column = 0; column < frame.Width; column++)
            {
                var luma = 298 * (lumaRow[column] - 16);
                var chromaU = chromaRow[column & ~1] - 128;
                var chromaV = chromaRow[(column & ~1) + 1] - 128;
                pixelRow[column * 4] = Clamp((luma + (409 * chromaV) + 128) >> 8);
                pixelRow[(column * 4) + 1] = Clamp((luma - (100 * chromaU) - (208 * chromaV) + 128) >> 8);
                pixelRow[(column * 4) + 2] = Clamp((luma + (516 * chromaU) + 128) >> 8);
                pixelRow[(column * 4) + 3] = 255;
            }
        }
        return bitmap;
    }

    /// <summary>Clamps a channel value to 0–255.</summary>
    private static byte Clamp(int value) => (byte)Math.Clamp(value, 0, 255);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Screen-share frame skipped: {Problem}")]
    private partial void LogFrameSkipped(string problem);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Screen-share frame {Width}x{Height} dropped: it cannot be encoded within {MaximumBytes} bytes")]
    private partial void LogFrameTooLarge(int width, int height, int maximumBytes);

    [LoggerMessage(Level = LogLevel.Information, Message = "Screen-share resolution changed from {OldWidth}x{OldHeight} to {NewWidth}x{NewHeight}")]
    private partial void LogResolutionChanged(int oldWidth, int oldHeight, int newWidth, int newHeight);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Screen-share frame skipped: encoding failed")]
    private partial void LogEncodingFailed(Exception exception);

    /// <summary>A coarse grid of luma and chroma samples used to detect slide changes.</summary>
    private sealed class Fingerprint
    {
        private readonly byte[] samples;

        private Fingerprint(int width, int height, byte[] samples)
        {
            Width = width;
            Height = height;
            this.samples = samples;
        }

        /// <summary>The frame width.</summary>
        public int Width { get; }

        /// <summary>The frame height.</summary>
        public int Height { get; }

        /// <summary>Samples Y, U, and V on a fixed grid.</summary>
        public static Fingerprint Of(RawVideoFrame frame)
        {
            var source = frame.Data.Span;
            var chromaPlaneOffset = frame.Stride * frame.Height;
            var samples = new byte[FingerprintColumns * FingerprintRows * 3];
            var index = 0;
            for (var gridRow = 0; gridRow < FingerprintRows; gridRow++)
            {
                var row = ((2 * gridRow) + 1) * frame.Height / (2 * FingerprintRows);
                for (var gridColumn = 0; gridColumn < FingerprintColumns; gridColumn++)
                {
                    var column = ((2 * gridColumn) + 1) * frame.Width / (2 * FingerprintColumns);
                    var chromaIndex = chromaPlaneOffset + ((row / 2) * frame.Stride) + (column & ~1);
                    samples[index++] = source[(row * frame.Stride) + column];
                    samples[index++] = source[chromaIndex];
                    samples[index++] = source[chromaIndex + 1];
                }
            }
            return new Fingerprint(frame.Width, frame.Height, samples);
        }

        /// <summary>Whether this frame differs visibly from <paramref name="other"/>.</summary>
        public bool DiffersFrom(Fingerprint other)
        {
            if ((Width, Height) != (other.Width, other.Height))
            {
                return true;
            }
            long totalDifference = 0;
            for (var index = 0; index < samples.Length; index++)
            {
                totalDifference += Math.Abs(samples[index] - other.samples[index]);
            }
            return (double)totalDifference / samples.Length > ChangeThreshold;
        }
    }
}
