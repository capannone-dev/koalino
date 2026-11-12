// Renders a meeting's transcript on its page: one block per segment with the speaker and start,
// marking a guessed speaker with "(?)", overlapping speech with a label, and unclear words in
// italics (#90, #91). Imported meetings without segments show their flat text. With a meeting ID,
// each segment's text can be corrected in place (#96), keeping its speaker and timing; the
// meeting is then no longer re-transcribed. A link to a moment (`#at-<ms>`, from a chat answer's
// source, #110) scrolls to the passage that moment falls in.

"use client";

import { useEffect, useId, useState } from "react";
import type { TranscriptSegment } from "@/lib/api";
import { editErrorMessage, editSegment } from "@/lib/edits-api";

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
            <em className="text-muted" title="Unclear word">
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
  const [current, setCurrent] = useState(segments ?? []);
  const [editing, setEditing] = useState<{ key: string; text: string } | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const fieldPrefix = useId();

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
          ? "Removed. This meeting won't be re-transcribed, so your corrections stay."
          : "Saved. This meeting won't be re-transcribed, so your corrections stay."
      );
    } catch (error) {
      setMessage(editErrorMessage(error));
    } finally {
      setBusy(false);
    }
  }

  if (current.length === 0) {
    return <p id="transcript">{transcript ?? "No transcript is available."}</p>;
  }
  return (
    <>
      <ol className="space-y-3">
        {current.map((segment) => {
          const speaker = segment.uncertain_speaker ? `${segment.speaker} (?)` : segment.speaker;
          const key = keyOf(segment);
          const at = formatOffset(segment.start_time_ms);
          const fieldId = `${fieldPrefix}-${key}`;
          return (
            <li key={key} id={`t-${segment.start_time_ms}`} className="scroll-mt-20">
              <p className="text-sm font-medium text-accent-text">
                <span title={segment.uncertain_speaker ? "The speaker is a guess" : undefined}>{`${speaker} · ${at}`}</span>
                {segment.overlapping_speech ? (
                  <span className="ml-2 text-xs text-warning" title="People talked at once here; words may be missing">
                    Overlapping speech
                  </span>
                ) : null}
              </p>
              {editing?.key === key ? (
                <div className="space-y-2">
                  <label htmlFor={fieldId} className="sr-only">
                    {`Text at ${at}`}
                  </label>
                  <textarea
                    id={fieldId}
                    className={fieldClass}
                    value={editing.text}
                    onChange={(event) => setEditing({ key, text: event.target.value })}
                  />
                  <div className="flex flex-wrap gap-2">
                    <button type="button" className={buttonClass} disabled={busy} onClick={() => void save(segment, editing.text)}>
                      Save
                    </button>
                    <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditing(null)}>
                      Cancel
                    </button>
                  </div>
                  <p className="text-sm text-muted">Leave it empty to remove this part.</p>
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
                      aria-label={`Edit the text at ${at}`}
                      title="Correct this text"
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
