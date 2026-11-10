/**
 * Specifies the extension's record of meetings it sent to Note Taker, used to match the Teams
 * transcript published afterwards to the right meeting.
 */
import { describe, expect, it } from "vitest";
import {
  RecordedMeeting,
  matchRecapToMeeting,
  newMeetingId,
  pendingTranscriptMeetings,
  pruneMeetings,
  recapRetryDue,
} from "../src/lib/meetings";

const HOUR = 3_600_000;
const NOV_9_16H = new Date(2026, 10, 9, 16, 0).getTime();

/** A meeting recorded from 16:10 to 16:21 on 9 November 2026. */
function weeklySync(overrides: Partial<RecordedMeeting> = {}): RecordedMeeting {
  return {
    meetingId: "browser-teams-1",
    title: "Weekly sync",
    startMs: NOV_9_16H + 10 * 60_000,
    endMs: NOV_9_16H + 21 * 60_000,
    teamsTranscriptSeen: true,
    transcriptFetched: false,
    ...overrides,
  };
}

describe("matchRecapToMeeting", () => {
  it("matches the same title at the same time", () => {
    const meetings = [weeklySync(), weeklySync({ meetingId: "browser-teams-2", startMs: NOV_9_16H + 26 * HOUR })];
    const card = { title: "Weekly sync", startMs: NOV_9_16H + 9 * 60_000, endMs: NOV_9_16H + 21 * 60_000 };
    expect(matchRecapToMeeting(card, meetings)?.meetingId).toBe("browser-teams-1");
  });

  it("matches by time when the tab showed no title", () => {
    const meetings = [weeklySync({ title: "" })];
    const card = { title: "Meeting with Ada", startMs: NOV_9_16H + 10 * 60_000, endMs: NOV_9_16H + 20 * 60_000 };
    expect(matchRecapToMeeting(card, meetings)?.meetingId).toBe("browser-teams-1");
  });

  it("does not match another meeting with the same title on another day", () => {
    const card = { title: "Weekly sync", startMs: NOV_9_16H + 7 * 24 * HOUR, endMs: NOV_9_16H + 7 * 24 * HOUR + HOUR };
    expect(matchRecapToMeeting(card, [weeklySync()])).toBeNull();
  });

  it("matches the latest meeting of that title when the card shows no times", () => {
    const meetings = [weeklySync(), weeklySync({ meetingId: "browser-teams-2", startMs: NOV_9_16H + 2 * HOUR })];
    expect(matchRecapToMeeting({ title: "weekly sync", startMs: null, endMs: null }, meetings)?.meetingId).toBe(
      "browser-teams-2",
    );
  });
});

describe("meeting records", () => {
  it("lists meetings whose Teams transcript is still to fetch", () => {
    const meetings = [weeklySync(), weeklySync({ meetingId: "done", transcriptFetched: true })];
    expect(pendingTranscriptMeetings(meetings).map((meeting) => meeting.meetingId)).toEqual(["browser-teams-1"]);
  });

  it("forgets meetings after two weeks", () => {
    const old = weeklySync({ meetingId: "old", startMs: NOV_9_16H - 15 * 24 * HOUR });
    expect(pruneMeetings([old, weeklySync()], NOV_9_16H).map((meeting) => meeting.meetingId)).toEqual([
      "browser-teams-1",
    ]);
  });

  it("makes unique, recognisable meeting IDs", () => {
    const first = newMeetingId(NOV_9_16H);
    expect(first).toMatch(/^browser-teams-\d+-[0-9a-f]{8}$/);
    expect(newMeetingId(NOV_9_16H)).not.toBe(first);
  });
});

describe("settings", async () => {
  const { DEFAULT_SETTINGS, normalizeSettings } = await import("../src/lib/settings");
  it("keeps known booleans and defaults the rest", () => {
    expect(normalizeSettings(null)).toEqual(DEFAULT_SETTINGS);
    expect(normalizeSettings({ alwaysRecordTeams: true, captureSlides: "yes", other: 1 })).toEqual({
      ...DEFAULT_SETTINGS,
      alwaysRecordTeams: true,
    });
  });
});

describe("recapRetryDue", () => {
  it("tries a recap at once, then again a minute after a failure, at most three times", () => {
    expect(recapRetryDue(undefined, 0)).toBe(true);
    expect(recapRetryDue({ count: 1, lastMs: 0 }, 30_000)).toBe(false);
    expect(recapRetryDue({ count: 1, lastMs: 0 }, 60_000)).toBe(true);
    expect(recapRetryDue({ count: 3, lastMs: 0 }, 10 * 60_000)).toBe(false);
  });
});
