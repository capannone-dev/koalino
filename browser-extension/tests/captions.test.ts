/**
 * Specifies how live captions and the live transcript panel become WebVTT documents.
 */
import { describe, expect, it } from "vitest";
import { CaptionCollector, TranscriptPanelCollector } from "../src/lib/captions";
import { buildVtt, formatVttTimestamp } from "../src/lib/vtt";

describe("WebVTT", () => {
  it("formats timestamps", () => {
    expect(formatVttTimestamp(0)).toBe("00:00:00.000");
    expect(formatVttTimestamp(3_723_045)).toBe("01:02:03.045");
  });

  it("writes cues with escaped speaker voices", () => {
    expect(
      buildVtt([{ startMs: 1_000, endMs: 2_500, speaker: "Ada <Lab>", text: "Hi & bye" }]),
    ).toBe("WEBVTT\n\n00:00:01.000 --> 00:00:02.500\n<v Ada &lt;Lab&gt;>Hi &amp; bye</v>\n");
  });
});

describe("CaptionCollector", () => {
  it("follows a caption as it grows and keeps its first time", () => {
    const collector = new CaptionCollector();
    const item = {};
    collector.observe([{ key: item, speaker: "Ada", speakerId: "8:orgid:a", text: "Good" }], 10_000);
    collector.observe([{ key: item, speaker: "Ada", speakerId: "8:orgid:a", text: "Good morning" }], 11_000);
    collector.observe(
      [{ key: item, speaker: "Ada", speakerId: "8:orgid:a", text: "Good morning, everyone." }],
      12_500,
    );
    expect(collector.cues()).toEqual([
      { startMs: 10_000, endMs: 12_500, speaker: "Ada", speakerId: "8:orgid:a", text: "Good morning, everyone." },
    ]);
  });

  it("starts a new cue when a recycled element shows other words", () => {
    const collector = new CaptionCollector();
    const item = {};
    collector.observe([{ key: item, speaker: "Ada", text: "First sentence here." }], 1_000);
    collector.observe([{ key: item, speaker: "Alan", text: "Something else entirely." }], 5_000);
    const cues = collector.cues();
    expect(cues.map((cue) => [cue.speaker, cue.startMs])).toEqual([
      ["Ada", 1_000],
      ["Alan", 5_000],
    ]);
  });

  it("keeps several speakers apart and in time order", () => {
    const collector = new CaptionCollector();
    const first = {};
    const second = {};
    collector.observe([{ key: first, speaker: "Ada", text: "Hello." }], 1_000);
    collector.observe(
      [
        { key: first, speaker: "Ada", text: "Hello." },
        { key: second, speaker: "Alan", text: "Hi." },
      ],
      2_000,
    );
    expect(collector.cues().map((cue) => cue.speaker)).toEqual(["Ada", "Alan"]);
    expect(collector.toVtt()).toContain("<v Alan>Hi.</v>");
    expect(collector.isEmpty()).toBe(false);
  });

  it("ignores empty captions", () => {
    const collector = new CaptionCollector();
    collector.observe([{ key: {}, speaker: "Ada", text: "   " }], 1_000);
    expect(collector.isEmpty()).toBe(true);
  });
});

describe("TranscriptPanelCollector", () => {
  it("keeps one entry per panel item, updated in place", () => {
    const collector = new TranscriptPanelCollector();
    collector.observe([{ id: "g/1", speaker: "Ada", text: "Hello" }], 3_000);
    collector.observe(
      [
        { id: "g/1", speaker: "Ada", text: "Hello there." },
        { id: "g/2", speaker: "Alan", text: "Hi." },
      ],
      6_000,
    );
    expect(collector.cues()).toEqual([
      { startMs: 3_000, endMs: 6_000, speaker: "Ada", text: "Hello there." },
      { startMs: 6_000, endMs: 6_000, speaker: "Alan", text: "Hi." },
    ]);
  });
});
