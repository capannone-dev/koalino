/**
 * The extension's record of the meetings it sent to Note Taker (kept in `chrome.storage.local`).
 *
 * Teams publishes its transcript in the meeting chat after the meeting, as a recap card showing
 * the meeting title and times. The record lets the content script match that card to the meeting
 * it recorded and send the transcript to the same Note Taker meeting. Records hold the title and
 * times only, and are forgotten after `KEEP_MS`.
 */

/** A meeting the extension recorded. */
export interface RecordedMeeting {
  /** The ID sent to Note Taker as the Teams meeting ID. */
  meetingId: string;
  /** The title the tab showed ("" when none). */
  title: string;
  /** When recording started and ended (epoch ms; `endMs` is null while it runs). */
  startMs: number;
  endMs: number | null;
  /** Whether Teams transcription was seen running during the meeting. */
  teamsTranscriptSeen: boolean;
  /** Whether the Teams transcript was already sent to Note Taker. */
  transcriptFetched: boolean;
}

/** How long a record is kept: Teams transcripts appear within minutes, but people look later. */
export const KEEP_MS = 14 * 24 * 3_600_000;
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
      const end = meeting.endMs ?? meeting.startMs;
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
 * The meetings whose Teams transcript has not been sent yet.
 * Inputs: The records. Outputs: Those not fetched. Side effects: None.
 */
export function pendingTranscriptMeetings(meetings: readonly RecordedMeeting[]): RecordedMeeting[] {
  return meetings.filter((meeting) => !meeting.transcriptFetched);
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
 * Inputs: The start time. Outputs: `browser-teams-<ms>-<8 hex>`. Side effects: Reads randomness.
 */
export function newMeetingId(startMs: number): string {
  const random = new Uint8Array(4);
  crypto.getRandomValues(random);
  const hex = [...random].map((byte) => byte.toString(16).padStart(2, "0")).join("");
  return `browser-teams-${startMs}-${hex}`;
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
