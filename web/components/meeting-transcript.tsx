// Renders a meeting's transcript on its page: one block per segment with the speaker and start,
// marking a guessed speaker with "(?)", overlapping speech with a label, and unclear words in
// italics (#90, #91). Imported meetings without segments show their flat text. With a meeting ID,
// each segment's text can be corrected in place (#96), keeping its speaker and timing; the
// meeting is then no longer re-transcribed. A link to a moment (`#at-<ms>`, from a chat answer's
// source, #110) scrolls to the passage that moment falls in. With a kept recording (#197), each
// passage plays from its start, and the passage being heard is marked and kept in view until the
// owner scrolls by hand.

"use client";

import { useEffect, useId, useState } from "react";
import type { TranscriptSegment } from "@/lib/api";
import { passageStartAt, useRecordingPlayback } from "@/lib/recording-playback";
import { editErrorMessage, editSegment } from "@/lib/edits-api";
import { useT } from "@/lib/i18n/client";

const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-3 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";
const fieldClass =
  "min-h-24 w-full rounded-lg border border-line-strong bg-surface px-3 py-2 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";

/** Formats an offset from the meeting start as `m:ss`, or `h:mm:ss` past an hour. */
function formatOffset(milliseconds: number): string {
  const totalSeconds = Math.floor(milliseconds / 1000);
  const hours = Math.floor(totalSeconds / 3600);
  const minutes = Math.floor((totalSeconds % 3600) / 60);
  const seconds = String(totalSeconds % 60).padStart(2, "0");
  return hours > 0 ? `${hours}:${String(minutes).padStart(2, "0")}:${seconds}` : `${minutes}:${seconds}`;
}

/** Shows a segment's words, the unclear ones in italics with a tooltip. */
function SegmentText({ segment }: { segment: TranscriptSegment }) {
  const t = useT();
  const unclear = new Set(segment.uncertain_words ?? []);
  if (unclear.size === 0) {
    return <p>{segment.text}</p>;
  }
  const words = segment.text.split(/\s+/).filter((word) => word.length > 0);
  return (
    <p>
      {words.map((word, position) => (
        <span key={position}>
          {position > 0 ? " " : null}
          {unclear.has(position) ? (
            <em className="text-muted" title={t("unclearWord")}>
              {word}
            </em>
          ) : (
            word
          )}
        </span>
      ))}
    </p>
  );
}

/** The key of a segment: its start and end. */
function keyOf(segment: { start_time_ms: number; end_time_ms: number }): string {
  return `${segment.start_time_ms}-${segment.end_time_ms}`;
}

/**
 * Lists the transcript.
 *
 * Inputs: The meeting's segments (absent or empty for an imported meeting), its flat text, and
 * its ID, which turns on correcting segments. Outputs: The transcript blocks, the flat text, or
 * "No transcript is available.". Side effects: Saving a correction writes it to the meeting.
 */
export function MeetingTranscript({
  segments,
  transcript,
  meetingId
}: {
  segments?: TranscriptSegment[];
  transcript: string | null;
  meetingId?: string;
}) {
  const t = useT();
  const [current, setCurrent] = useState(segments ?? []);
  const [editing, setEditing] = useState<{ key: string; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const fieldPrefix = useId();
  const playback = useRecordingPlayback();
  const player = playback?.available ? playback : null;
  const heardStart =
    player && (player.playing || player.positionMs > 0)
      ? passageStartAt(
          current.map((segment) => segment.start_time_ms),
          player.positionMs
        )
      : null;
  const following = Boolean(player?.playing && player.follow);

  // Keeps the passage being heard in view while following.
  useEffect(() => {
    if (!following || heardStart === null) return;
    document.getElementById(`t-${heardStart}`)?.scrollIntoView?.({ block: "center", behavior: "smooth" });
  }, [following, heardStart]);

  // Scrolling by hand while it plays stops following; the player offers to follow again.
  const stopFollowing = player?.setFollow;
  useEffect(() => {
    if (!following || !stopFollowing) return;
    const stop = () => stopFollowing(false);
    const keys = (event: KeyboardEvent) => {
      if (["PageUp", "PageDown", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)) stop();
    };
    window.addEventListener("wheel", stop, { passive: true });
    window.addEventListener("touchmove", stop, { passive: true });
    window.addEventListener("keydown", keys);
    return () => {
      window.removeEventListener("wheel", stop);
      window.removeEventListener("touchmove", stop);
      window.removeEventListener("keydown", keys);
    };
  }, [following, stopFollowing]);

  // Scrolls to the passage a cited moment falls in: the last one starting at or before it.
  useEffect(() => {
    const cited = /^#at-(\d+)$/.exec(window.location.hash);
    if (!cited) return;
    const moment = Number(cited[1]);
    const passage = current.filter((segment) => segment.start_time_ms <= moment).at(-1) ?? current[0];
    if (passage) {
      document.getElementById(`t-${passage.start_time_ms}`)?.scrollIntoView({ block: "start" });
    }
    // Only when the page opens; later edits must not scroll.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  /** Saves the correction of `segment`. */
  async function save(segment: TranscriptSegment, text: string) {
    if (!meetingId) return;
    setBusy(true);
    setMessage(null);
    try {
      const saved = await editSegment(meetingId, segment.start_time_ms, segment.end_time_ms, text);
      setCurrent((segments) =>
        saved === null
          ? segments.filter((candidate) => keyOf(candidate) !== keyOf(segment))
          : segments.map((candidate) =>
              keyOf(candidate) === keyOf(segment) ? { ...candidate, text: saved.text, uncertain_words: [] } : candidate
            )
      );
      setEditing(null);
      setMessage(
        saved === null
          ? t("removedThisMeetingWontBe")
          : t("savedThisMeetingWontBe")
      );
    } catch (error) {
      setMessage(editErrorMessage(error, t));
    } finally {
      setBusy(false);
    }
  }

  if (current.length === 0) {
    return <p id="transcript">{transcript ?? t("noTranscriptIsAvailable")}</p>;
  }
  return (
    <>
      {meetingId ? <TranscriptTip /> : null}
      <ol className="space-y-3">
        {current.map((segment) => {
          const speaker = segment.uncertain_speaker ? `${segment.speaker} (?)` : segment.speaker;
          const key = keyOf(segment);
          const at = formatOffset(segment.start_time_ms);
          const fieldId = `${fieldPrefix}-${key}`;
          return (
            <li
              key={key}
              id={`t-${segment.start_time_ms}`}
              aria-current={heardStart === segment.start_time_ms ? "true" : undefined}
              className={`scroll-mt-20 rounded-lg ${heardStart === segment.start_time_ms ? "bg-accent-soft px-2 py-1" : ""}`}
            >
              <p className="flex flex-wrap items-center gap-x-2 text-sm font-medium text-accent-text">
                <span title={segment.uncertain_speaker ? t("theSpeakerIsAGuess") : undefined}>{`${speaker} · ${at}`}</span>
                {player ? (
                  <button
                    type="button"
                    className="inline-flex min-h-8 min-w-8 items-center justify-center rounded-full hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus"
                    aria-label={t("playFromAt", { at })}
                    title={t("playFromAt", { at })}
                    onClick={() => player.playFrom(segment.start_time_ms)}
                  >
                    <svg aria-hidden="true" viewBox="0 0 24 24" width="16" height="16" fill="currentColor">
                      <path d="M8 5v14l11-7z" />
                    </svg>
                  </button>
                ) : null}
                {segment.overlapping_speech ? (
                  <span className="text-xs text-warning" title={t("peopleTalkedAtOnceHere")}>{t("overlappingSpeech")}</span>
                ) : null}
              </p>
              {editing?.key === key ? (
                <div className="space-y-2">
                  <label htmlFor={fieldId} className="sr-only">
                    {t("textAt", { at })}
                  </label>
                  <textarea
                    id={fieldId}
                    className={fieldClass}
                    value={editing.text}
                    onChange={(event) => setEditing({ key, text: event.target.value })}
                  />
                  <div className="flex flex-wrap gap-2">
                    <button type="button" className={buttonClass} disabled={busy} onClick={() => void save(segment, editing.text)}>{t("save")}</button>
                    <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditing(null)}>{t("cancel")}</button>
                  </div>
                  <p className="text-sm text-muted">{t("leaveItEmptyToRemove")}</p>
                </div>
              ) : (
                <div className="flex items-start gap-2">
                  <div className="flex-1">
                    <SegmentText segment={segment} />
                  </div>
                  {meetingId ? (
                    <button
                      type="button"
                      className={buttonClass}
                      aria-label={t("editTheTextAt", { at })}
                      title={t("correctThisText")}
                      disabled={busy}
                      onClick={() => setEditing({ key, text: segment.text })}
                    >
                      <svg aria-hidden="true" viewBox="0 0 24 24" width="18" height="18" fill="none" stroke="currentColor" strokeWidth="2">
                        <path d="M12 20h9" />
                        <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
                      </svg>
                    </button>
                  ) : null}
                </div>
              )}
            </li>
          );
        })}
      </ol>
      {meetingId ? (
        <p role="status" aria-live="polite">
          {message}
        </p>
      ) : null}
    </>
  );
}

/** Where this browser remembers that the transcript tip was dismissed (#213). */
export const TRANSCRIPT_TIP_KEY = "koalino.tip.transcript-corrections";

/**
 * The one-time tip over a transcript (#213): words in grey italics are ones Koalino was unsure
 * of, and the pencil corrects a passage. Shown after mount, so the server's page never differs
 * from the first render, and only until dismissed in this browser.
 *
 * Side effects: reads and writes `localStorage`; without it the tip shows each time.
 */
function TranscriptTip() {
  const t = useT();
  const [show, setShow] = useState(false);
  useEffect(() => {
    let seen = false;
    try {
      seen = window.localStorage.getItem(TRANSCRIPT_TIP_KEY) === "seen";
    } catch {
      seen = false;
    }
    setShow(!seen);
  }, []);
  if (!show) return null;
  return (
    <aside className="mb-4 flex flex-wrap items-start gap-3 rounded-xl border border-line bg-accent-soft p-4 text-sm" aria-label={t("tip")}>
      <p className="flex-1">
        {t("tipUnsureWordsBefore")}
        <em className="text-muted">{t("tipUnsureWordsExample")}</em>
        {t("tipUnsureWordsAfter")}
      </p>
      <button
        type="button"
        className="min-h-11 rounded-lg px-3 font-medium text-accent-text hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-focus"
        onClick={() => {
          setShow(false);
          try {
            window.localStorage.setItem(TRANSCRIPT_TIP_KEY, "seen");
          } catch {
            // Without storage the tip simply returns next time.
          }
        }}
      >
        {t("gotIt")}
      </button>
    </aside>
  );
}
