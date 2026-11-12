/**
 * Specifies the one-line summary the popup shows for a recording in progress: what was sent to
 * Koalino so far, and the last problem (an error name, never page content).
 */
import { describe, expect, it } from "vitest";
import { describeProgress } from "../src/lib/messages";

describe("describeProgress", () => {
  it("counts what was sent", () => {
    expect(
      describeProgress({ title: "Weekly sync", audioChunks: 12, slides: 1, captions: 30, problem: null }),
    ).toBe("Weekly sync: 12 audio chunks, 1 slide, 30 captions");
  });

  it("names an untitled meeting and adds the last problem", () => {
    expect(
      describeProgress({ title: "", audioChunks: 1, slides: 0, captions: 0, problem: "slides: SecurityError" }),
    ).toBe("Teams meeting: 1 audio chunk, 0 slides, 0 captions (problem: slides: SecurityError)");
  });
});
