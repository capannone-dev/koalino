/**
 * Turns captured browser audio into what the Koalino core takes: 16 kHz mono signed 16-bit
 * PCM, cut into chunks of `CHUNK_MS` stamped with their time from the meeting start.
 *
 * Pure functions with no browser APIs, so they run in the page hook and in tests alike. Audio is
 * never logged.
 */

/** The sample rate the core requires. */
export const TARGET_SAMPLE_RATE = 16_000;
/** Length of one chunk sent to the core (it accepts 1-30 s). */
export const CHUNK_MS = 10_000;
/** Shortest remainder worth sending when a track stops. */
const MINIMUM_FLUSH_MS = 1_000;
/** Below this RMS (about -55 dBFS) a chunk is silence, such as a muted microphone. */
const SILENCE_RMS = 60;

/**
 * Converts float samples at `sampleRate` to 16 kHz signed 16-bit samples.
 *
 * Inputs: Mono samples in -1..1 (values outside are clamped) and their rate (16 kHz or more).
 * Outputs: The 16 kHz samples; each output sample averages the input samples it covers, which
 * also filters out frequencies above 8 kHz well enough for speech.
 * Side effects: None.
 */
export function resampleTo16k(input: Float32Array, sampleRate: number): Int16Array {
  const ratio = sampleRate / TARGET_SAMPLE_RATE;
  const outputLength = Math.floor(input.length / ratio);
  const output = new Int16Array(outputLength);
  for (let outputIndex = 0; outputIndex < outputLength; outputIndex += 1) {
    const start = Math.floor(outputIndex * ratio);
    const end = Math.max(start + 1, Math.floor((outputIndex + 1) * ratio));
    let sum = 0;
    for (let inputIndex = start; inputIndex < end; inputIndex += 1) sum += input[inputIndex] ?? 0;
    const value = Math.max(-1, Math.min(1, sum / (end - start)));
    output[outputIndex] = value < 0 ? Math.round(value * 32768) : Math.round(value * 32767);
  }
  return output;
}

/** One chunk ready for the core. */
export interface PcmChunk {
  /** Milliseconds from the meeting start to the chunk's first sample. */
  timestampMs: number;
  /** 16 kHz samples. */
  samples: Int16Array;
}

/**
 * Collects 16 kHz samples and emits `CHUNK_MS` chunks.
 */
export class PcmChunker {
  private pending: Int16Array[] = [];
  private pendingLength = 0;
  private emittedSamples = 0;

  /**
   * Inputs: The meeting time of the first sample (when the track started) and the receiver of
   * full chunks. Side effects: None.
   */
  constructor(
    private readonly startMs: number,
    private readonly onChunk: (chunk: PcmChunk) => void,
  ) {}

  /** Adds samples; emits every chunk they complete. Side effects: Calls `onChunk`. */
  push(samples: Int16Array): void {
    this.pending.push(samples);
    this.pendingLength += samples.length;
    const chunkSamples = (CHUNK_MS / 1000) * TARGET_SAMPLE_RATE;
    while (this.pendingLength >= chunkSamples) this.emit(chunkSamples);
  }

  /** Emits what is left when it holds at least a second. Side effects: May call `onChunk`. */
  flush(): void {
    if (this.pendingLength >= (MINIMUM_FLUSH_MS / 1000) * TARGET_SAMPLE_RATE) {
      this.emit(this.pendingLength);
    }
    this.pending = [];
    this.pendingLength = 0;
  }

  /** Emits the first `length` pending samples as one chunk. */
  private emit(length: number): void {
    const merged = new Int16Array(this.pendingLength);
    let offset = 0;
    for (const part of this.pending) {
      merged.set(part, offset);
      offset += part.length;
    }
    const chunk = merged.slice(0, length);
    const rest = merged.slice(length);
    this.pending = rest.length > 0 ? [rest] : [];
    this.pendingLength = rest.length;
    const timestampMs = this.startMs + Math.round((this.emittedSamples / TARGET_SAMPLE_RATE) * 1000);
    this.emittedSamples += length;
    this.onChunk({ timestampMs, samples: chunk });
  }
}

/**
 * Tells whether a chunk is silence (a muted microphone, or no one speaking remotely).
 *
 * Inputs: 16-bit samples. Outputs: Whether their RMS is below about -55 dBFS. Side effects: None.
 */
export function isSilent(samples: Int16Array): boolean {
  if (samples.length === 0) return true;
  let sumOfSquares = 0;
  for (const sample of samples) sumOfSquares += sample * sample;
  return Math.sqrt(sumOfSquares / samples.length) < SILENCE_RMS;
}

/**
 * Encodes bytes as standard base64 (native messaging carries JSON only).
 *
 * Inputs: The bytes. Outputs: The base64 text. Side effects: None.
 */
export function base64FromBytes(bytes: Uint8Array): string {
  let binary = "";
  const step = 0x8000;
  for (let offset = 0; offset < bytes.length; offset += step) {
    binary += String.fromCharCode(...bytes.subarray(offset, offset + step));
  }
  return btoa(binary);
}

/**
 * Little-endian bytes of 16-bit samples, as the core expects.
 *
 * Inputs: The samples. Outputs: Their bytes. Side effects: None.
 */
export function bytesFromSamples(samples: Int16Array): Uint8Array {
  const bytes = new Uint8Array(samples.length * 2);
  const view = new DataView(bytes.buffer);
  samples.forEach((sample, index) => view.setInt16(index * 2, sample, true));
  return bytes;
}
