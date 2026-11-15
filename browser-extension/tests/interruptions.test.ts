/**
 * Specifies how the background ends a recording whose tab went away mid-call (#102): it waits
 * for the tab to come back from a reload, and ends the meeting in Koalino only if it does not.
 */
import { describe, expect, it } from "vitest";
import { InterruptedRecordings } from "../src/lib/interruptions";
import { RESUME_GAP_MS } from "../src/lib/meetings";

/** Interruptions over a fake timer queue, with the meetings it ended. */
function fixture() {
  const ended: string[] = [];
  const timers = new Map<number, { at: number; run: () => void }>();
  let clock = 0;
  let nextId = 1;
  const interruptions = new InterruptedRecordings((meetingId) => ended.push(meetingId), {
    setTimeout: (run, delayMs) => {
      const id = nextId++;
      timers.set(id, { at: clock + delayMs, run });
      return id;
    },
    clearTimeout: (id) => timers.delete(id),
  });
  const advance = (ms: number) => {
    clock += ms;
    for (const [id, timer] of [...timers]) {
      if (timer.at <= clock) {
        timers.delete(id);
        timer.run();
      }
    }
  };
  return { interruptions, ended, advance };
}

describe("InterruptedRecordings", () => {
  it("ends a meeting whose tab did not come back", () => {
    const { interruptions, ended, advance } = fixture();
    interruptions.interrupted("browser-teams-1");
    advance(RESUME_GAP_MS - 1);
    expect(ended).toEqual([]);
    advance(1);
    expect(ended).toEqual(["browser-teams-1"]);
  });

  it("leaves a meeting alone once its tab resumed it", () => {
    const { interruptions, ended, advance } = fixture();
    interruptions.interrupted("browser-teams-1");
    advance(20_000);
    interruptions.resumed("browser-teams-1");
    advance(RESUME_GAP_MS);
    expect(ended).toEqual([]);
  });

  it("waits again from the latest interruption", () => {
    const { interruptions, ended, advance } = fixture();
    interruptions.interrupted("browser-teams-1");
    advance(60_000);
    interruptions.interrupted("browser-teams-1");
    advance(RESUME_GAP_MS - 1);
    expect(ended).toEqual([]);
    advance(1);
    expect(ended).toEqual(["browser-teams-1"]);
  });
});
