// Formats a meeting's details for its page: when it was created, the speech-to-text languages
// chosen while recording it, and the devices it was recorded on, worded as the apps word them.

import type { Combining, RecordedOn, RecordingDevice } from "@/lib/api";
import { englishT, type Translate } from "@/lib/i18n";

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

/**
 * Words a meeting's time for people (#210), in the browser's time zone and the page's language:
 * "Today 9:32 AM", "Yesterday …", the weekday within the last week, "Oct 28 …" within this year,
 * else "Mar 2, 2025 …".
 *
 * Inputs: the daemon's `created_at`; the page language; `now` (for tests).
 * Outputs: the text, or the input unchanged when it is not a date. Side effects: none.
 */
export function formatMeetingWhen(createdAt: string, language: string, now: Date = new Date()): string {
  const instant = new Date(createdAt);
  if (Number.isNaN(instant.getTime())) return createdAt;
  const time = new Intl.DateTimeFormat(language, { hour: "numeric", minute: "2-digit" }).format(instant);
  // Whole calendar days between the two local dates, immune to daylight-saving shifts.
  const dayNumber = (date: Date) => Date.UTC(date.getFullYear(), date.getMonth(), date.getDate()) / 86_400_000;
  const daysAgo = dayNumber(now) - dayNumber(instant);
  if (daysAgo === 0 || daysAgo === 1) {
    const word = new Intl.RelativeTimeFormat(language, { numeric: "auto" }).format(-daysAgo, "day");
    return `${word.charAt(0).toLocaleUpperCase(language)}${word.slice(1)} ${time}`;
  }
  if (daysAgo > 1 && daysAgo < 7) {
    return `${new Intl.DateTimeFormat(language, { weekday: "short" }).format(instant)} ${time}`;
  }
  const sameYear = instant.getFullYear() === now.getFullYear();
  const date = new Intl.DateTimeFormat(language, sameYear ? { month: "short", day: "numeric" } : { year: "numeric", month: "short", day: "numeric" }).format(instant);
  return `${date} ${time}`;
}

/** Whisper codes that are not standard language tags, named as the apps name them. */
const whisperOnlyNames: Record<string, string> = { jw: "Javanese" };

/**
 * Names one Whisper language code in English: `auto` is `Automatic`; an unknown code stays as-is.
 *
 * Side effects: none.
 */
export function transcriptionLanguageName(code: string, t: Translate = englishT): string {
  if (code === "auto") return t("automatic");
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
export function describeTranscriptionLanguages(codes: readonly string[] | undefined, t: Translate = englishT): string | null {
  if (codes === undefined || codes.length === 0) return null;
  const heading = codes.length === 1 ? t("transcriptionLanguage") : t("transcriptionLanguages");
  return `${heading}: ${codes.map((code) => transcriptionLanguageName(code, t)).join(", ")}`;
}

/**
 * Names the devices a combined meeting was recorded on (#88), as the apps do.
 *
 * Inputs: the daemon's `recorded_on` and `combined_recordings` (missing from older daemons).
 * Outputs: `Recorded on Laptop + Pixel` (an unknown device is `this device` or `another device`);
 * `null` for a meeting no other recording was combined into. Side effects: none.
 */
export function describeRecordedOn(meeting: RecordedOn, t: Translate = englishT): string | null {
  if (!meeting.combined_recordings?.length || !meeting.recorded_on?.length) return null;
  const names = meeting.recorded_on.map((device) => device.name ?? (device.this_device ? t("thisDevice2") : t("anotherDevice")));
  return t("recordedOn", { names: names.join(" + ") });
}

/** A device's name with "(not connected)" when a paired device is away (#88). */
function deviceLabel(device: RecordingDevice, withConnection: boolean, t: Translate = englishT): string {
  const name = device.name ?? (device.this_device ? t("thisDevice2") : t("anotherDevice"));
  return withConnection && device.connected === false ? t("notConnected", { name }) : name;
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
export function describeCombining(combining: Combining | undefined, t: Translate = englishT): string | null {
  if (combining === undefined) return null;
  const names = combining.waiting_for.map((device) => deviceLabel(device, true, t)).join(", ");
  const plainNames = combining.waiting_for.map((device) => deviceLabel(device, false, t)).join(", ");
  const combiner = combining.combine_on === null ? t("anotherDevice") : deviceLabel(combining.combine_on, true, t);
  const kept = combining.kept_until === null ? "" : t("itIsKeptEncryptedUntil", { calendarDay: calendarDay(combining.kept_until) });
  switch (combining.state) {
    case "one_device":
      return combining.kept_until === null
        ? t("recordedOnThisDeviceOnly")
        : t("recordedOnThisDeviceOnly2", { calendarDay: calendarDay(combining.kept_until) });
    case "waiting":
      if (combining.waiting_for.length === 0) return t("combinesTheRecordingsNothingIs", { combiner });
      if (combining.combine_on?.this_device) return t("waitingForAudioFrom", { names, kept });
      return t("combinesTheRecordingsWaitingFor", { combiner, names });
    case "ready_to_combine":
      return t("audioFromEveryDeviceIs");
    case "sending":
      return t("thisDevicesAudioStillHas", { combiner, kept });
    case "combined":
      return t("transcriptCombinedFromEveryDevice");
    case "not_combined":
      return t("notCombinedAudioFromDid", { plainNames });
  }
}
