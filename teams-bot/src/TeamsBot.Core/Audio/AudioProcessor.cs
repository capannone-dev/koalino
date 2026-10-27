// Turns unmixed Teams audio frames into per-participant 16 kHz mono PCM chunks for the Rust app.
//
// Frames are buffered per media source (speaker). A chunk ends when the speaker has been silent
// (no frames) for longer than SilenceGapMs, when it reaches MaximumChunkDurationMs (at most the
// 30 s the Rust app accepts), or when the caller flushes on stream stop or meeting end. Shorter
// gaps are filled with silence so chunk length matches real time. The participant is resolved
// through the roster when a chunk is emitted; unknown sources get a stable placeholder ID.

using Microsoft.Extensions.Logging;
using TeamsBot.Core.Abstractions;
using TeamsBot.Core.Bridge;
using TeamsBot.Core.Model;

namespace TeamsBot.Core.Audio;

/// <summary>Chunk boundaries (configuration section <c>TeamsBot:Audio</c>).</summary>
public sealed class AudioChunkingOptions
{
    /// <summary>Longest chunk in milliseconds (1–30000; default 10000).</summary>
    public int MaximumChunkDurationMs { get; set; } = 10_000;

    /// <summary>Silence after which a speaker's chunk ends, in milliseconds (default 600).</summary>
    public int SilenceGapMs { get; set; } = 600;

    /// <summary>Rejects values the Rust app or the chunker cannot handle.</summary>
    /// <exception cref="ArgumentException">A value is out of range.</exception>
    public void Validate()
    {
        if (MaximumChunkDurationMs < 1 || (ulong)MaximumChunkDurationMs > ProtobufSerializer.MaximumChunkDurationMs)
        {
            throw new ArgumentException("MaximumChunkDurationMs must be between 1 and 30000.", nameof(MaximumChunkDurationMs));
        }
        if (SilenceGapMs < 1)
        {
            throw new ArgumentException("SilenceGapMs must be positive.", nameof(SilenceGapMs));
        }
    }
}

/// <summary>Buffers and chunks unmixed audio per speaker. Thread-safe.</summary>
public sealed partial class AudioProcessor
{
    private readonly AudioChunkingOptions options;
    private readonly IParticipantDirectory participantDirectory;
    private readonly ILogger<AudioProcessor> logger;
    private readonly Dictionary<uint, SpeakerBuffer> speakerBuffers = new();
    private readonly HashSet<uint> reportedUnknownSources = new();
    private readonly object gate = new();

    /// <summary>Creates a processor.</summary>
    /// <param name="options">Chunk limits; validated here.</param>
    /// <param name="participantDirectory">Resolves media source IDs to participants.</param>
    /// <param name="logger">Receives warnings (never audio).</param>
    /// <exception cref="ArgumentException">The options are invalid.</exception>
    public AudioProcessor(AudioChunkingOptions options, IParticipantDirectory participantDirectory, ILogger<AudioProcessor> logger)
    {
        ArgumentNullException.ThrowIfNull(options);
        options.Validate();
        this.options = options;
        this.participantDirectory = participantDirectory ?? throw new ArgumentNullException(nameof(participantDirectory));
        this.logger = logger ?? throw new ArgumentNullException(nameof(logger));
    }

    /// <summary>Adds one frame and returns any chunks it completed.</summary>
    /// <param name="frame">An unmixed buffer; malformed buffers are skipped with a warning.</param>
    /// <returns>Completed chunks, oldest first (often empty).</returns>
    public IReadOnlyList<PcmAudioChunk> ProcessFrame(UnmixedAudioFrame frame)
    {
        ArgumentNullException.ThrowIfNull(frame);
        byte[] pcm;
        try
        {
            pcm = PcmConverter.ToPcm16kMono(frame.Pcm, frame.Format);
        }
        catch (ArgumentException exception)
        {
            LogFrameSkipped(frame.SourceId, exception.Message);
            return Array.Empty<PcmAudioChunk>();
        }
        if (pcm.Length == 0)
        {
            return Array.Empty<PcmAudioChunk>();
        }

        var completed = new List<PcmAudioChunk>();
        lock (gate)
        {
            if (speakerBuffers.TryGetValue(frame.SourceId, out var buffer))
            {
                var bufferEndMs = buffer.EndMs;
                if (frame.OffsetMs > bufferEndMs + (ulong)options.SilenceGapMs)
                {
                    completed.Add(Emit(frame.SourceId, buffer));
                    speakerBuffers.Remove(frame.SourceId);
                }
                else if (frame.OffsetMs > bufferEndMs)
                {
                    Append(frame.SourceId, new byte[checked((int)(frame.OffsetMs - bufferEndMs) * PcmAudioChunk.BytesPerMillisecond)], completed);
                }
            }
            if (!speakerBuffers.ContainsKey(frame.SourceId))
            {
                speakerBuffers[frame.SourceId] = new SpeakerBuffer(frame.OffsetMs);
            }
            Append(frame.SourceId, pcm, completed);
        }
        return completed;
    }

    /// <summary>Ends the chunks of speakers that have been silent longer than the silence gap.</summary>
    /// <param name="nowOffsetMs">The current time in milliseconds since the meeting start.</param>
    /// <returns>The ended chunks.</returns>
    public IReadOnlyList<PcmAudioChunk> FlushIdle(ulong nowOffsetMs)
    {
        lock (gate)
        {
            var idleSources = speakerBuffers
                .Where(entry => nowOffsetMs > entry.Value.EndMs + (ulong)options.SilenceGapMs)
                .Select(entry => entry.Key)
                .ToList();
            return idleSources.Select(FlushSource).ToList();
        }
    }

    /// <summary>Ends every pending chunk (audio stream stopped or meeting ended).</summary>
    /// <returns>The ended chunks.</returns>
    public IReadOnlyList<PcmAudioChunk> FlushAll()
    {
        lock (gate)
        {
            return speakerBuffers.Keys.ToList().Select(FlushSource).ToList();
        }
    }

    /// <summary>Removes and emits one speaker's buffer. Caller holds the lock.</summary>
    private PcmAudioChunk FlushSource(uint sourceId)
    {
        var buffer = speakerBuffers[sourceId];
        speakerBuffers.Remove(sourceId);
        return Emit(sourceId, buffer);
    }

    /// <summary>Appends PCM to a speaker's buffer, emitting full chunks. Caller holds the lock.</summary>
    private void Append(uint sourceId, ReadOnlySpan<byte> pcm, List<PcmAudioChunk> completed)
    {
        var maximumBytes = options.MaximumChunkDurationMs * PcmAudioChunk.BytesPerMillisecond;
        while (!pcm.IsEmpty)
        {
            if (!speakerBuffers.TryGetValue(sourceId, out var buffer))
            {
                return;
            }
            var takenByteCount = Math.Min(pcm.Length, maximumBytes - (int)buffer.Pcm.Length);
            buffer.Pcm.Write(pcm[..takenByteCount]);
            pcm = pcm[takenByteCount..];
            if (buffer.Pcm.Length >= maximumBytes)
            {
                completed.Add(Emit(sourceId, buffer));
                speakerBuffers[sourceId] = new SpeakerBuffer(buffer.StartMs + (ulong)options.MaximumChunkDurationMs);
            }
        }
    }

    /// <summary>Builds a chunk for a speaker buffer, resolving its participant.</summary>
    private PcmAudioChunk Emit(uint sourceId, SpeakerBuffer buffer)
    {
        if (!participantDirectory.TryResolve(sourceId, out var participant))
        {
            participant = new ParticipantInfo($"teams-media-source-{sourceId}", $"Unknown participant {sourceId}");
            if (reportedUnknownSources.Add(sourceId))
            {
                LogUnknownSource(sourceId);
            }
        }
        return new PcmAudioChunk(participant, buffer.Pcm.ToArray(), buffer.StartMs);
    }

    [LoggerMessage(Level = LogLevel.Warning, Message = "Audio frame from media source {SourceId} skipped: {Reason}")]
    private partial void LogFrameSkipped(uint sourceId, string reason);

    [LoggerMessage(Level = LogLevel.Warning, Message = "Audio from media source {SourceId} has no roster participant; using a placeholder identity")]
    private partial void LogUnknownSource(uint sourceId);

    /// <summary>One speaker's pending audio.</summary>
    private sealed class SpeakerBuffer(ulong startMs)
    {
        /// <summary>Chunk start in milliseconds since the meeting start.</summary>
        public ulong StartMs { get; } = startMs;

        /// <summary>The buffered 16 kHz mono PCM.</summary>
        public MemoryStream Pcm { get; } = new();

        /// <summary>Chunk end in milliseconds since the meeting start.</summary>
        public ulong EndMs => StartMs + (ulong)(Pcm.Length / PcmAudioChunk.BytesPerMillisecond);
    }
}
