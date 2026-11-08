// Renders a meeting's transcript on its page: one block per segment with the speaker and start,
// marking a guessed speaker with "(?)", overlapping speech with a label, and unclear words in
// italics (#90, #91). Imported meetings without segments show their flat text.

import type { TranscriptSegment } from "@/lib/api";

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

/**
 * Lists the transcript.
 *
 * Inputs: The meeting's segments (absent or empty for an imported meeting) and its flat text.
 * Outputs: The transcript blocks, the flat text, or "No transcript is available.".
 * Side effects: None.
 */
export function MeetingTranscript({ segments, transcript }: { segments?: TranscriptSegment[]; transcript: string | null }) {
  if (!segments || segments.length === 0) {
    return <p>{transcript ?? "No transcript is available."}</p>;
  }
  return (
    <ol className="space-y-3">
      {segments.map((segment) => {
        const speaker = segment.uncertain_speaker ? `${segment.speaker} (?)` : segment.speaker;
        return (
          <li key={`${segment.start_time_ms}-${segment.end_time_ms}`}>
            <p className="text-sm font-medium text-accent-text">
              <span title={segment.uncertain_speaker ? "The speaker is a guess" : undefined}>
                {`${speaker} · ${formatOffset(segment.start_time_ms)}`}
              </span>
              {segment.overlapping_speech ? (
                <span className="ml-2 text-xs text-warning" title="People talked at once here; words may be missing">
                  Overlapping speech
                </span>
              ) : null}
            </p>
            <SegmentText segment={segment} />
          </li>
        );
      })}
    </ol>
  );
}
