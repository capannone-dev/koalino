import { englishT, type Translate } from "@/lib/i18n";
// Validates meeting titles in the browser with the same rules Rust's `MeetingTitle::parse` enforces.

/** Longest title Rust accepts, in Unicode characters (`MeetingTitle::MAX_CHARACTERS`). */
export const maxMeetingTitleCharacters = 200;

/** The trimmed title to save, and why it is invalid (`null` when it is valid). */
export type MeetingTitleValidation = { trimmedTitle: string; error: string | null };

/** Unicode general category Cc (U+0000–U+001F, U+007F–U+009F), which Rust's `char::is_control` matches. */
const controlCharacterPattern = /\p{Cc}/u;

/**
 * Validates `rawTitle` exactly as Rust will, so invalid titles are explained without a request.
 *
 * Inputs: the text as typed, possibly with surrounding whitespace.
 * Outputs: the trimmed title and the first broken rule's message, checked in Rust's order: empty,
 * too long (counted in code points, so an emoji counts once, as Rust counts `char`s), control
 * character. Side effects: none.
 */
export function validateMeetingTitle(rawTitle: string, t: Translate = englishT): MeetingTitleValidation {
  const trimmedTitle = rawTitle.trim();
  const characterCount = Array.from(trimmedTitle).length;
  let error: string | null = null;
  if (trimmedTitle.length === 0) {
    error = t("enterATitle");
  } else if (characterCount > maxMeetingTitleCharacters) {
    error = t("titlesCanBeAtMost", { maxMeetingTitleCharacters, characterCount });
  } else if (controlCharacterPattern.test(trimmedTitle)) {
    error = t("titlesCantContainLineBreaks");
  }
  return { trimmedTitle, error };
}
