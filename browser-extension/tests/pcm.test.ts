/**
 * Specifies how captured browser audio becomes the 16 kHz PCM chunks the Note Taker core takes.
 */
import { describe, expect, it } from "vitest";
import {
  CHUNK_MS,
  PcmChunker,
  base64FromBytes,
  isSilent,
  resampleTo16k,
} from "../src/lib/pcm";

describe("resampleTo16k", () => {
  it("averages 48 kHz down to 16 kHz", () => {
    const input = new Float32Array([0.3, 0.3, 0.3, -0.6, -0.6, -0.6]);
    const output = resampleTo16k(input, 48_000);
    expect(Array.from(output)).toEqual([
      Math.round(Math.fround(0.3) * 32767),
      Math.round(Math.fround(-0.6) * 32768),
    ]);
  });

  it("clamps samples outside -1..1", () => {
    const output = resampleTo16k(new Float32Array([2, -2]), 16_000);
    expect(Array.from(output)).toEqual([32767, -32768]);
  });

  it("handles a rate that is not a multiple of 16 kHz", () => {
    const output = resampleTo16k(new Float32Array(44_100).fill(0.5), 44_100);
    expect(output.length).toBe(16_000);
  });
});

describe("PcmChunker", () => {
  it("emits fixed chunks stamped with their meeting time", () => {
    const chunks: { timestampMs: number; samples: Int16Array }[] = [];
    const chunker = new PcmChunker(5_000, (chunk) => chunks.push(chunk));
    const oneSecond = new Int16Array(16_000).fill(100);
    for (let second = 0; second < 25; second += 1) chunker.push(oneSecond);
    expect(chunks.map((chunk) => chunk.timestampMs)).toEqual([5_000, 5_000 + CHUNK_MS]);
    expect(chunks[0]?.samples.length).toBe((CHUNK_MS / 1000) * 16_000);
  });

  it("flushes the rest when stopped, dropping less than a second", () => {
    const chunks: { timestampMs: number; samples: Int16Array }[] = [];
    const chunker = new PcmChunker(0, (chunk) => chunks.push(chunk));
    chunker.push(new Int16Array(24_000).fill(1));
    chunker.flush();
    expect(chunks).toHaveLength(1);
    expect(chunks[0]?.samples.length).toBe(24_000);

    const short: unknown[] = [];
    const shortChunker = new PcmChunker(0, (chunk) => short.push(chunk));
    shortChunker.push(new Int16Array(8_000));
    shortChunker.flush();
    expect(short).toHaveLength(0);
  });
});

describe("isSilent", () => {
  it("treats a muted microphone as silence but not quiet speech", () => {
    expect(isSilent(new Int16Array(16_000))).toBe(true);
    expect(isSilent(new Int16Array(16_000).fill(3))).toBe(true);
    expect(isSilent(new Int16Array(16_000).fill(300))).toBe(false);
  });
});

describe("base64FromBytes", () => {
  it("encodes like btoa", () => {
    const bytes = new Uint8Array([0, 1, 2, 250, 255]);
    expect(base64FromBytes(bytes)).toBe(btoa(String.fromCharCode(...bytes)));
  });
});
