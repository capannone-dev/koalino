/**
 * Writes WebVTT, the format Teams itself uses for transcripts, so captions, the live transcript
 * panel and the downloaded Teams transcript all reach the core in one format.
 */

/** One timed line of speech. */
export interface VttCue {
  /** Milliseconds from the meeting start. */
  startMs: number;
  /** Milliseconds from the meeting start. */
  endMs: number;
  /** The speaker's display name. */
  speaker: string;
  /** What was said. */
  text: string;
}

/**
 * Formats milliseconds as `HH:MM:SS.mmm`.
 *
 * Inputs: Non-negative milliseconds. Outputs: The timestamp. Side effects: None.
 */
export function formatVttTimestamp(milliseconds: number): string {
  const total = Math.max(0, Math.round(milliseconds));
  const hours = Math.floor(total / 3_600_000);
  const minutes = Math.floor((total % 3_600_000) / 60_000);
  const seconds = Math.floor((total % 60_000) / 1000);
  const millis = total % 1000;
  const pad = (value: number, width = 2) => String(value).padStart(width, "0");
  return `${pad(hours)}:${pad(minutes)}:${pad(seconds)}.${pad(millis, 3)}`;
}

/** Escapes text for a cue payload. */
function escapeCueText(text: string): string {
  return text.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

/**
 * Writes a WebVTT document with one voice-tagged cue per line of speech.
 *
 * Inputs: Cues in display order. Outputs: The document text. Side effects: None.
 */
export function buildVtt(cues: readonly VttCue[]): string {
  const blocks = cues.map(
    (cue) =>
      `${formatVttTimestamp(cue.startMs)} --> ${formatVttTimestamp(Math.max(cue.endMs, cue.startMs))}\n` +
      `<v ${escapeCueText(cue.speaker)}>${escapeCueText(cue.text)}</v>\n`,
  );
  return ["WEBVTT\n", ...blocks].join("\n");
}
