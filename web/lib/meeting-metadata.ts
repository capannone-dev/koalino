// Formats a meeting's details for its page: when it was created, and the speech-to-text languages
// chosen while recording it, worded as the desktop and Android apps word them.

/**
 * Formats an RFC 3339 instant as `YYYY-MM-DD HH:MM UTC`, as the apps and untitled meetings' titles do.
 *
 * Inputs: the daemon's `created_at`. Outputs: the text, or the input unchanged when it is not a
 * date. Side effects: none.
 */
export function formatMeetingCreatedAt(createdAt: string): string {
  const instant = new Date(createdAt);
  if (Number.isNaN(instant.getTime())) return createdAt;
  const twoDigits = (value: number) => String(value).padStart(2, "0");
  return (
    `${String(instant.getUTCFullYear()).padStart(4, "0")}-${twoDigits(instant.getUTCMonth() + 1)}-` +
    `${twoDigits(instant.getUTCDate())} ${twoDigits(instant.getUTCHours())}:${twoDigits(instant.getUTCMinutes())} UTC`
  );
}

/** Whisper codes that are not standard language tags, named as the apps name them. */
const whisperOnlyNames: Record<string, string> = { jw: "Javanese" };

/**
 * Names one Whisper language code in English: `auto` is `Automatic`; an unknown code stays as-is.
 *
 * Side effects: none.
 */
export function transcriptionLanguageName(code: string): string {
  if (code === "auto") return "Automatic";
  const whisperOnlyName = whisperOnlyNames[code];
  if (whisperOnlyName !== undefined) return whisperOnlyName;
  try {
    return new Intl.DisplayNames(["en"], { type: "language", fallback: "code" }).of(code) ?? code;
  } catch {
    // A malformed tag: show what the daemon sent.
    return code;
  }
}

/**
 * Describes a meeting's speech-to-text languages, in first-use order.
 *
 * Inputs: the daemon's `transcription_languages` (missing from older daemons).
 * Outputs: `Transcription language: Italian` or `Transcription languages: Italian, English`;
 * `null` when none was recorded. Side effects: none.
 */
export function describeTranscriptionLanguages(codes: readonly string[] | undefined): string | null {
  if (codes === undefined || codes.length === 0) return null;
  const heading = codes.length === 1 ? "Transcription language" : "Transcription languages";
  return `${heading}: ${codes.map(transcriptionLanguageName).join(", ")}`;
}
