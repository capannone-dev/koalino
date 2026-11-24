/**
 * Specifies the extension's record of meetings it sent to Koalino, used to match the Teams
 * transcript published afterwards to the right meeting.
 */
import { describe, expect, it } from "vitest";
import {
  HEARTBEAT_MS,
  RESUME_GAP_MS,
  RecordedMeeting,
  heartbeatDue,
  closestSession,
  matchRecapToMeeting,
  matchRecapViewToMeeting,
  newMeetingId,
  pendingTranscriptMeetings,
  pruneMeetings,
  recapRetryDue,
  resumableMeeting,
  leftMeetingToClose,
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

describe("a page reloaded during the call (#102)", () => {
  /** A two-hour call whose page reloaded at 18:00: no end, last seen then. */
  const reloaded = weeklySync({ startMs: NOV_9_16H, endMs: null, lastSeenMs: NOV_9_16H + 2 * HOUR });
  const lateCard = { title: "Weekly sync", startMs: NOV_9_16H + 100 * 60_000, endMs: NOV_9_16H + 2 * HOUR };

  it("matches by the last time the recording was seen", () => {
    expect(matchRecapToMeeting(lateCard, [reloaded])?.meetingId).toBe("browser-teams-1");
  });

  it("would not match from the start alone", () => {
    const { lastSeenMs: _dropped, ...withoutHeartbeat } = reloaded;
    expect(matchRecapToMeeting(lateCard, [withoutHeartbeat])).toBeNull();
  });

  it("notes a running recording every half minute", () => {
    const running = weeklySync({ startMs: NOV_9_16H, endMs: null });
    expect(heartbeatDue(running, NOV_9_16H + HEARTBEAT_MS - 1)).toBe(false);
    expect(heartbeatDue(running, NOV_9_16H + HEARTBEAT_MS)).toBe(true);
    const noted = { ...running, lastSeenMs: NOV_9_16H + HEARTBEAT_MS };
    expect(heartbeatDue(noted, NOV_9_16H + HEARTBEAT_MS + 1_000)).toBe(false);
    expect(heartbeatDue(weeklySync(), NOV_9_16H + HOUR), "an ended recording").toBe(false);
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

describe("resuming after a reload (#102)", () => {
  /** A recording this tab was running, last seen at 17:00. */
  const interrupted = weeklySync({ startMs: NOV_9_16H, endMs: null, lastSeenMs: NOV_9_16H + HOUR });
  const seenAt = NOV_9_16H + HOUR;

  it("resumes the recording this tab ran when the call is back within two minutes", () => {
    expect(resumableMeeting([interrupted], "browser-teams-1", seenAt + RESUME_GAP_MS)?.meetingId).toBe("browser-teams-1");
  });

  it("starts afresh once the gap is longer", () => {
    expect(resumableMeeting([interrupted], "browser-teams-1", seenAt + RESUME_GAP_MS + 1)).toBeNull();
  });

  it("never resumes a recording that ended, or one another tab ran", () => {
    expect(resumableMeeting([weeklySync({ endMs: seenAt })], "browser-teams-1", seenAt + 1_000)).toBeNull();
    expect(resumableMeeting([interrupted], "browser-teams-2", seenAt + 1_000)).toBeNull();
    expect(resumableMeeting([interrupted], null, seenAt + 1_000)).toBeNull();
  });

  it("starts afresh when this tab joined another call, but not when the title is unknown", () => {
    expect(resumableMeeting([interrupted], "browser-teams-1", seenAt + 1_000, "Board review")).toBeNull();
    expect(resumableMeeting([interrupted], "browser-teams-1", seenAt + 1_000, " weekly  SYNC ")?.meetingId).toBe("browser-teams-1");
    expect(resumableMeeting([interrupted], "browser-teams-1", seenAt + 1_000, "")?.meetingId).toBe("browser-teams-1");
  });

  it("measures the gap from the start when no heartbeat was noted", () => {
    const young = weeklySync({ startMs: seenAt, endMs: null });
    expect(resumableMeeting([young], "browser-teams-1", seenAt + 10_000)?.meetingId).toBe("browser-teams-1");
  });
});

describe("leftMeetingToClose (a call left in the page, 2026-11-20)", () => {
  const leftAt = NOV_9_16H + 30 * 60_000;
  const left = weeklySync({ endMs: null, lastSeenMs: leftAt });

  it("keeps a left call open while it may be rejoined", () => {
    expect(leftMeetingToClose([left], "browser-teams-1", leftAt + RESUME_GAP_MS)).toBeNull();
  });

  it("closes it once the gap passed, ending it when it was last seen", () => {
    expect(leftMeetingToClose([left], "browser-teams-1", leftAt + RESUME_GAP_MS + 1)).toEqual({ ...left, endMs: leftAt });
  });

  it("leaves alone ended meetings and other tabs' meetings", () => {
    expect(leftMeetingToClose([weeklySync()], "browser-teams-1", leftAt + HOUR)).toBeNull();
    expect(leftMeetingToClose([left], "browser-teams-2", leftAt + HOUR)).toBeNull();
    expect(leftMeetingToClose([left], null, leftAt + HOUR)).toBeNull();
  });
});

describe("closestSession (#158)", () => {
  const at = (hours: number, minutes: number) => new Date(2026, 10, 24, hours, minutes).getTime();
  const earlier = { name: "earlier", startMs: at(14, 24), endMs: at(15, 5) };
  const recorded = { name: "recorded", startMs: at(17, 16), endMs: at(17, 18) };
  const recording = { startMs: at(17, 11), endMs: at(17, 18) };

  it("picks the session the recording overlaps, not an earlier one", () => {
    expect(closestSession([earlier, recorded], recording)?.name).toBe("recorded");
    expect(closestSession([recorded, earlier], recording)?.name).toBe("recorded");
  });

  it("prefers the larger overlap between back-to-back sessions", () => {
    const first = { name: "first", startMs: at(10, 0), endMs: at(10, 30) };
    const second = { name: "second", startMs: at(10, 30), endMs: at(11, 0) };
    expect(closestSession([first, second], { startMs: at(10, 25), endMs: at(11, 0) })?.name).toBe("second");
  });

  it("finds no session far from the recording or without times", () => {
    expect(closestSession([earlier], recording)).toBeNull();
    expect(closestSession([{ name: "untimed", startMs: null, endMs: null }], recording)).toBeNull();
  });
});

describe("matchRecapViewToMeeting (#158)", () => {
  // The chat's recap view shows one session; its picker lists the others.
  const earlierSession = { title: "Weekly sync", startMs: NOV_9_16H - 3 * HOUR, endMs: NOV_9_16H - 2 * HOUR };

  it("matches a recording of the same meeting whatever session the view shows", () => {
    expect(matchRecapViewToMeeting(earlierSession, [weeklySync()])?.meetingId).toBe("browser-teams-1");
  });

  it("prefers the recording the shown session overlaps", () => {
    const shown = { title: "Weekly sync", startMs: NOV_9_16H, endMs: NOV_9_16H + HOUR };
    const later = weeklySync({ meetingId: "browser-teams-2", startMs: NOV_9_16H + 5 * HOUR, endMs: NOV_9_16H + 6 * HOUR });
    expect(matchRecapViewToMeeting(shown, [later, weeklySync()])?.meetingId).toBe("browser-teams-1");
  });

  it("matches by title only recordings that saw Teams transcription", () => {
    expect(matchRecapViewToMeeting(earlierSession, [weeklySync({ teamsTranscriptSeen: false })])).toBeNull();
    expect(matchRecapViewToMeeting({ ...earlierSession, title: "Other" }, [weeklySync()])).toBeNull();
  });
});
