// Specifies how the meeting page words a meeting's creation time and transcription languages.

import { describe, expect, it, test } from "vitest";
import { describeRecordedOn, describeTranscriptionLanguages, formatMeetingCreatedAt, transcriptionLanguageName } from "@/lib/meeting-metadata";

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
