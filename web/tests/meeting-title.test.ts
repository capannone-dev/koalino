// Verifies browser-side meeting title validation matches Rust's `MeetingTitle::parse` rules.

import { expect, test } from "vitest";
import { maxMeetingTitleCharacters, validateMeetingTitle } from "@/lib/meeting-title";

test("trims surrounding whitespace before accepting a title", () => {
  expect(validateMeetingTitle("  Project Sync \t")).toEqual({ trimmedTitle: "Project Sync", error: null });
});

test.each(["", "   ", "\t\n "])("rejects the blank title %j", (rawTitle) => {
  expect(validateMeetingTitle(rawTitle).error).toBe("Enter a title.");
});

test("accepts exactly 200 characters and rejects 201 with the count", () => {
  expect(maxMeetingTitleCharacters).toBe(200);
  expect(validateMeetingTitle("a".repeat(200)).error).toBeNull();
  expect(validateMeetingTitle("a".repeat(201)).error).toBe("Titles can be at most 200 characters (you have 201).");
});

test("counts characters as Unicode code points, so an emoji counts once", () => {
  expect(validateMeetingTitle("🎉".repeat(200)).error).toBeNull();
  expect(validateMeetingTitle("🎉".repeat(201)).error).toBe("Titles can be at most 200 characters (you have 201).");
});

test.each(["Weekly\nsync", "Weekly\tsync", "Weekly\u0007sync", "Weekly\u0085sync", "Weekly\u007fsync"])(
  "rejects the control character in %j",
  (rawTitle) => {
    expect(validateMeetingTitle(rawTitle).error).toBe("Titles can't contain line breaks or control characters.");
  }
);

test("accepts CJK and emoji titles", () => {
  expect(validateMeetingTitle("项目周会 🎉")).toEqual({ trimmedTitle: "项目周会 🎉", error: null });
});
