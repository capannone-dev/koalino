/**
 * The extension's record of the meetings it sent to Koalino (kept in `chrome.storage.local`).
 *
 * Teams publishes its transcript in the meeting chat after the meeting, as a recap card showing
 * the meeting title and times. The record lets the content script match that card to the meeting
 * it recorded and send the transcript to the same Koalino meeting. Records hold the title and
 * times only, and are forgotten after `KEEP_MS`.
 */

/** A meeting the extension recorded. */
export interface RecordedMeeting {
  /** The ID sent to Koalino as the Teams meeting ID. */
  meetingId: string;
  /** The title the tab showed ("" when none). */
  title: string;
  /** When recording started and ended (epoch ms; `endMs` is null while it runs). */
  startMs: number;
  endMs: number | null;
  /**
   * The last time the recording was seen running (epoch ms), noted every `HEARTBEAT_MS`. A page
   * reloaded during the call never writes `endMs`, so this stands in for it (#102).
   */
  lastSeenMs?: number;
  /** Whether Teams transcription was seen running during the meeting. */
  teamsTranscriptSeen: boolean;
  /** Whether the Teams transcript was already sent to Koalino. */
  transcriptFetched: boolean;
}

/** How long a record is kept: Teams transcripts appear within minutes, but people look later. */
export const KEEP_MS = 14 * 24 * 3_600_000;
/** How often a running recording notes that it is still running. */
export const HEARTBEAT_MS = 30_000;
/**
 * How long a call may be gone (a reload, a rejoin) and still continue the same Koalino meeting
 * (#102). Measured from the recording's last heartbeat, so a reload has about 1.5–2 minutes.
 */
export const RESUME_GAP_MS = 2 * 60_000;
/** How far a recap's times may differ from the recorded times. */
const TIME_SLACK_MS = 30 * 60_000;

/** Normalizes a title for comparison. */
function normalized(title: string): string {
  return title.replace(/\s+/g, " ").trim().toLowerCase();
}

/**
 * Finds the recorded meeting a recap card belongs to.
 *
 * Inputs: The card's title and times (null when not shown) and the records.
 * Outputs: With times: the meeting overlapping them (with 30 min slack), preferring the same
 * title; without times: the latest meeting with that title. `null` when none fits.
 * Side effects: None.
 */
export function matchRecapToMeeting(
  card: { title: string; startMs: number | null; endMs: number | null },
  meetings: readonly RecordedMeeting[],
): RecordedMeeting | null {
  const title = normalized(card.title);
  if (card.startMs !== null) {
    const cardStart = card.startMs;
    const cardEnd = card.endMs ?? card.startMs;
    const overlapping = meetings.filter((meeting) => {
      const end = recordedEnd(meeting);
      return meeting.startMs <= cardEnd + TIME_SLACK_MS && end >= cardStart - TIME_SLACK_MS;
    });
    return (
      overlapping.find((meeting) => normalized(meeting.title) === title) ??
      (overlapping.length === 1 ? (overlapping[0] ?? null) : null)
    );
  }
  const sameTitle = meetings
    .filter((meeting) => normalized(meeting.title) === title)
    .sort((first, second) => second.startMs - first.startMs);
  return sameTitle[0] ?? null;
}

/**
 * Finds the recorded meeting a chat's recap view belongs to (#158).
 *
 * The view shows one session at a time, and its session picker lists the others, so a recording
 * of the same meeting may belong to a session not shown (`fetchRecapTranscript` then chooses it).
 *
 * Teams also shows the view before its session picker, so with no times shown yet the title is
 * all there is to go by (#159).
 *
 * Inputs: The view's title and shown session times (null when not shown), and the records.
 * Outputs: The meeting the shown session overlaps (`matchRecapToMeeting`), else the latest
 * recording with the same title that saw Teams transcription (only those can have a transcript);
 * `null` when none fits.
 * Side effects: None.
 */
export function matchRecapViewToMeeting(
  view: { title: string; startMs: number | null; endMs: number | null },
  meetings: readonly RecordedMeeting[],
): RecordedMeeting | null {
  const transcribed = meetings.filter((meeting) => meeting.teamsTranscriptSeen);
  const byTitle = { title: view.title, startMs: null, endMs: null };
  if (view.startMs === null) return matchRecapToMeeting(byTitle, transcribed);
  return matchRecapToMeeting(view, meetings) ?? matchRecapToMeeting(byTitle, transcribed);
}

/**
 * Picks the recap session a recording belongs to (#158).
 *
 * A meeting chat keeps a recap per session, and Teams may open it on any of them, so the session
 * shown must be checked against the recording before its transcript is taken.
 *
 * Inputs: The sessions (times null when not shown) and the recording's start and end (epoch ms).
 * Outputs: The session nearest the recording, within the recap slack (30 min): the one it
 * overlaps most, else the one with the smallest gap. `null` when none is near or none has times.
 * Side effects: None.
 */
export function closestSession<Session extends { startMs: number | null; endMs: number | null }>(
  sessions: readonly Session[],
  recording: { startMs: number; endMs: number },
): Session | null {
  let best: { session: Session; gapMs: number; overlapMs: number } | null = null;
  for (const session of sessions) {
    if (session.startMs === null) continue;
    const sessionEnd = session.endMs ?? session.startMs;
    const gapMs = Math.max(0, session.startMs - recording.endMs, recording.startMs - sessionEnd);
    if (gapMs > TIME_SLACK_MS) continue;
    const overlapMs = Math.max(0, Math.min(sessionEnd, recording.endMs) - Math.max(session.startMs, recording.startMs));
    if (!best || gapMs < best.gapMs || (gapMs === best.gapMs && overlapMs > best.overlapMs)) {
      best = { session, gapMs, overlapMs };
    }
  }
  return best?.session ?? null;
}

/**
 * When a recording ended: its end, else the last time it was seen running, else its start.
 * Inputs: The record. Outputs: Epoch ms. Side effects: None.
 */
export function recordedEnd(meeting: RecordedMeeting): number {
  return meeting.endMs ?? meeting.lastSeenMs ?? meeting.startMs;
}

/**
 * Whether a running recording should note that it is still running.
 * Inputs: The record and the time now. Outputs: `true` once `HEARTBEAT_MS` passed since it was
 * last noted (or started); `false` for an ended recording. Side effects: None.
 */
export function heartbeatDue(meeting: RecordedMeeting, nowMs: number): boolean {
  if (meeting.endMs !== null) return false;
  return nowMs - (meeting.lastSeenMs ?? meeting.startMs) >= HEARTBEAT_MS;
}

/**
 * The recording a reloaded tab should continue instead of starting a new meeting (#102).
 *
 * Inputs: The records; the meeting ID this tab was recording before it reloaded (kept in the
 * tab's `sessionStorage`, `null` when none); the time now; and the title the tab shows now.
 * Outputs: That record when it never ended, was last seen at most `RESUME_GAP_MS` ago, and has
 * the same title (when both are known); otherwise `null`. Only the tab that recorded a meeting
 * continues it, so a call in another tab, or another call joined in this one, stays its own.
 * Side effects: None.
 */
export function resumableMeeting(
  meetings: readonly RecordedMeeting[],
  tabMeetingId: string | null,
  nowMs: number,
  currentTitle = "",
): RecordedMeeting | null {
  if (tabMeetingId === null) return null;
  const meeting = meetings.find((candidate) => candidate.meetingId === tabMeetingId);
  if (!meeting || meeting.endMs !== null) return null;
  const [recordedTitle, shownTitle] = [normalized(meeting.title), normalized(currentTitle)];
  if (recordedTitle !== "" && shownTitle !== "" && recordedTitle !== shownTitle) return null;
  return nowMs - recordedEnd(meeting) <= RESUME_GAP_MS ? meeting : null;
}

/**
 * The meeting this tab left without ending it, now past the time to rejoin (2026-11-20): leaving
 * the call in the page keeps the meeting open for `RESUME_GAP_MS`, so a dropped call that is
 * rejoined continues it, as a reload does (#102).
 *
 * Inputs: The records; the meeting ID this tab remembers (`null` when none); the time now.
 * Outputs: That record with its end set to when it was last seen, once the gap passed;
 * otherwise `null`. Side effects: None.
 */
export function leftMeetingToClose(
  meetings: readonly RecordedMeeting[],
  tabMeetingId: string | null,
  nowMs: number,
): RecordedMeeting | null {
  if (tabMeetingId === null) return null;
  const meeting = meetings.find((candidate) => candidate.meetingId === tabMeetingId);
  if (!meeting || meeting.endMs !== null) return null;
  const lastSeen = recordedEnd(meeting);
  return nowMs - lastSeen > RESUME_GAP_MS ? { ...meeting, endMs: lastSeen } : null;
}

/**
 * The meetings whose Teams transcript has not been sent yet.
 * Inputs: The records. Outputs: Those not fetched. Side effects: None.
 */
export function pendingTranscriptMeetings(meetings: readonly RecordedMeeting[]): RecordedMeeting[] {
  // Only Teams meetings have a Teams transcript; other sites' IDs name their site (#174).
  return meetings.filter((meeting) => !meeting.transcriptFetched && !/^browser-(?!teams-)/.test(meeting.meetingId));
}

/**
 * Drops records older than `KEEP_MS`.
 * Inputs: The records and the current time. Outputs: The records to keep. Side effects: None.
 */
export function pruneMeetings(meetings: readonly RecordedMeeting[], nowMs: number): RecordedMeeting[] {
  return meetings.filter((meeting) => nowMs - meeting.startMs <= KEEP_MS);
}

/**
 * A new meeting ID, unique across browsers and restarts.
 * Inputs: The start time, and the site's key (`siteKey`; Teams when left out, #174).
 * Outputs: `browser-<site>-<ms>-<8 hex>`, the site reduced to letters, digits and dashes.
 * Side effects: Reads randomness.
 */
export function newMeetingId(startMs: number, site = "teams"): string {
  const random = new Uint8Array(4);
  crypto.getRandomValues(random);
  const hex = [...random].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  const safeSite = site.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "") || "web";
  return `browser-${safeSite}-${startMs}-${hex}`;
}

/** The tries at fetching one meeting's recap transcript in this tab. */
export interface RecapAttempts {
  count: number;
  /** When the last try started (epoch ms). */
  lastMs: number;
}

/** Most tries at fetching one meeting's recap transcript per page load. */
const RECAP_MAX_ATTEMPTS = 3;
/** The pause after a failed try (the recap frame may still have been loading). */
const RECAP_RETRY_MS = 60_000;

/**
 * Whether to try fetching a meeting's recap transcript now.
 *
 * Inputs: The earlier tries (none yet: `undefined`) and the time now.
 * Outputs: `true` for a first try, or a retry a minute after the last, up to three tries.
 * Side effects: None.
 */
export function recapRetryDue(attempts: RecapAttempts | undefined, nowMs: number): boolean {
  if (!attempts) return true;
  return attempts.count < RECAP_MAX_ATTEMPTS && nowMs - attempts.lastMs >= RECAP_RETRY_MS;
}
