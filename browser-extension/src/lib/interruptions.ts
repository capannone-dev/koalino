/**
 * Ends recordings whose tab went away mid-call, unless the tab comes back (#102).
 *
 * A page that unloads during a call may be reloading, and the reloaded page continues the same
 * Koalino meeting. So the content script no longer ends the meeting when its page unloads; the
 * background notices the tab's port closing and waits `RESUME_GAP_MS`. If no tab resumes the
 * meeting by then (the tab was closed, the owner left), it tells Koalino the meeting ended, so
 * the after-meeting pass still runs.
 */
import { RESUME_GAP_MS } from "./meetings";

/** The timer functions, injected so tests can drive time. */
export interface TimerPorts {
  setTimeout(run: () => void, delayMs: number): number;
  clearTimeout(timerId: number): void;
}

/** Tracks interrupted recordings by meeting ID. */
export class InterruptedRecordings {
  /** Pending ends, by meeting ID. */
  private readonly pending = new Map<string, number>();

  /**
   * Inputs: What ends a meeting in Koalino (sends `ended` to the host), and the timers.
   * Side effects: None until a recording is interrupted.
   */
  constructor(
    private readonly endMeeting: (meetingId: string) => void,
    private readonly timers: TimerPorts,
  ) {}

  /**
   * A recording's tab went away mid-call: end its meeting unless it resumes in time.
   * Side effects: (Re)starts the meeting's timer; the timer calls `endMeeting`.
   */
  interrupted(meetingId: string): void {
    this.cancel(meetingId);
    const timerId = this.timers.setTimeout(() => {
      this.pending.delete(meetingId);
      this.endMeeting(meetingId);
    }, RESUME_GAP_MS);
    this.pending.set(meetingId, timerId);
  }

  /**
   * A tab (re)started recording this meeting: it is not ended.
   * Side effects: Cancels the meeting's timer, if any.
   */
  resumed(meetingId: string): void {
    this.cancel(meetingId);
  }

  /** Cancels a meeting's pending end. */
  private cancel(meetingId: string): void {
    const timerId = this.pending.get(meetingId);
    if (timerId === undefined) return;
    this.timers.clearTimeout(timerId);
    this.pending.delete(meetingId);
  }
}
