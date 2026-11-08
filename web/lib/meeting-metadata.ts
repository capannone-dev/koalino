// Formats a meeting's details for its page: when it was created, the speech-to-text languages
// chosen while recording it, and the devices it was recorded on, worded as the apps word them.

import type { Combining, RecordedOn, RecordingDevice } from "@/lib/api";

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

/**
 * Names the devices a combined meeting was recorded on (#88), as the apps do.
 *
 * Inputs: the daemon's `recorded_on` and `combined_recordings` (missing from older daemons).
 * Outputs: `Recorded on Laptop + Pixel` (an unknown device is `this device` or `another device`);
 * `null` for a meeting no other recording was combined into. Side effects: none.
 */
export function describeRecordedOn(meeting: RecordedOn): string | null {
  if (!meeting.combined_recordings?.length || !meeting.recorded_on?.length) return null;
  const names = meeting.recorded_on.map((device) => device.name ?? (device.this_device ? "this device" : "another device"));
  return `Recorded on ${names.join(" + ")}`;
}

/** A device's name with "(not connected)" when a paired device is away (#88). */
function deviceLabel(device: RecordingDevice, withConnection: boolean): string {
  const name = device.name ?? (device.this_device ? "this device" : "another device");
  return withConnection && device.connected === false ? `${name} (not connected)` : name;
}

/** The UTC calendar day of an RFC 3339 instant, as `YYYY-MM-DD`. */
function calendarDay(instant: string): string {
  return new Date(instant).toISOString().slice(0, 10);
}

/**
 * Words how far combining a meeting's recordings got, as the Flutter app does (#88).
 *
 * Inputs: the meeting's `combining`, absent from older daemons. Outputs: one or two sentences,
 * or null without a status. Side effects: none.
 */
export function describeCombining(combining: Combining | undefined): string | null {
  if (combining === undefined) return null;
  const names = combining.waiting_for.map((device) => deviceLabel(device, true)).join(", ");
  const plainNames = combining.waiting_for.map((device) => deviceLabel(device, false)).join(", ");
  const combiner = combining.combine_on === null ? "another device" : deviceLabel(combining.combine_on, true);
  const kept = combining.kept_until === null ? "" : ` It is kept encrypted until ${calendarDay(combining.kept_until)}.`;
  switch (combining.state) {
    case "one_device":
      return combining.kept_until === null
        ? "Recorded on this device only."
        : `Recorded on this device only. A recording of it made on another device can still be combined until ${calendarDay(combining.kept_until)}.`;
    case "waiting":
      if (combining.waiting_for.length === 0) return `${combiner} combines the recordings; nothing is left to send from here.`;
      if (combining.combine_on?.this_device) return `Waiting for audio from ${names}.${kept}`;
      return `${combiner} combines the recordings; waiting for audio from ${names}.`;
    case "ready_to_combine":
      return "Audio from every device is here; the transcript is combined when this device is free.";
    case "sending":
      return `This device's audio still has to reach ${combiner}.${kept}`;
    case "combined":
      return "Transcript combined from every device.";
    case "not_combined":
      return `Not combined: audio from ${plainNames} did not arrive within three weeks.`;
  }
}
