// Specifies how the meeting page words a meeting's creation time and transcription languages.

import { describe, expect, it, test } from "vitest";
import type { Combining } from "@/lib/api";
import { describeCombining, describeRecordedOn, describeTranscriptionLanguages, formatMeetingCreatedAt, transcriptionLanguageName } from "@/lib/meeting-metadata";

describe("meeting metadata", () => {
  it("formats the creation time in UTC as the apps do", () => {
    expect(formatMeetingCreatedAt("2026-10-27T09:05:30+00:00")).toBe("2026-10-27 09:05 UTC");
    expect(formatMeetingCreatedAt("2026-10-27T01:05:00+02:00")).toBe("2026-10-26 23:05 UTC");
    expect(formatMeetingCreatedAt("not a date")).toBe("not a date");
  });

  it("names Whisper's languages in English", () => {
    expect(transcriptionLanguageName("it")).toBe("Italian");
    expect(transcriptionLanguageName("auto")).toBe("Automatic");
    expect(transcriptionLanguageName("jw")).toBe("Javanese");
    expect(transcriptionLanguageName("not a tag!")).toBe("not a tag!");
  });

  it("describes one or several languages, and nothing without any", () => {
    expect(describeTranscriptionLanguages(["it"])).toBe("Transcription language: Italian");
    expect(describeTranscriptionLanguages(["it", "en"])).toBe("Transcription languages: Italian, English");
    expect(describeTranscriptionLanguages([])).toBeNull();
    expect(describeTranscriptionLanguages(undefined)).toBeNull();
  });
});

test("names the devices of a combined meeting, and nothing for one device (#88)", () => {
  expect(
    describeRecordedOn({
      recorded_on: [
        { device_id: "device-laptop", name: "Laptop", this_device: true },
        { device_id: "device-phone", name: null, this_device: false }
      ],
      combined_recordings: ["meeting-phone"]
    })
  ).toBe("Recorded on Laptop + another device");
  expect(
    describeRecordedOn({ recorded_on: [{ device_id: "d", name: null, this_device: true }], combined_recordings: [] })
  ).toBeNull();
  expect(describeRecordedOn({})).toBeNull();
});

test("words how far combining a meeting's recordings got (#88)", () => {
  const laptop = { device_id: "device-laptop", name: "Laptop", this_device: true, connected: null };
  const pixel = { device_id: "device-phone", name: "Pixel", this_device: false, connected: false };
  const until = "2026-11-23T15:00:00+00:00";
  const describe = (state: Combining["state"], extra: Partial<Combining> = {}) =>
    describeCombining({ state, combine_on: laptop, waiting_for: [], kept_until: null, ...extra });

  expect(describe("one_device", { combine_on: null })).toBe("Recorded on this device only.");
  expect(describe("one_device", { combine_on: null, kept_until: until })).toBe(
    "Recorded on this device only. A recording of it made on another device can still be combined until 2026-11-23."
  );
  expect(describe("waiting", { waiting_for: [pixel], kept_until: until })).toBe(
    "Waiting for audio from Pixel (not connected). It is kept encrypted until 2026-11-23."
  );
  expect(describe("waiting", { combine_on: { ...pixel, connected: true }, waiting_for: [] })).toBe(
    "Pixel combines the recordings; nothing is left to send from here."
  );
  expect(describe("ready_to_combine")).toBe(
    "Audio from every device is here; the transcript is combined when this device is free."
  );
  expect(describe("sending", { combine_on: { ...laptop, this_device: false, connected: false }, kept_until: until })).toBe(
    "This device's audio still has to reach Laptop (not connected). It is kept encrypted until 2026-11-23."
  );
  expect(describe("combined")).toBe("Transcript combined from every device.");
  expect(describe("not_combined", { waiting_for: [pixel] })).toBe(
    "Not combined: audio from Pixel did not arrive within three weeks."
  );
  expect(describeCombining(undefined)).toBeNull();
});
