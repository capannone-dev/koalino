/**
 * Reads a Teams meeting from the page (Issue #102): whether a call is on, its title, the live
 * captions, the transcript panel, whether Teams transcription runs, the shared screen, and the
 * recap cards Teams posts in the meeting chat afterwards.
 *
 * Teams has no public DOM contract, so each reader names the `data-tid` attributes the #102 probes
 * found (work account on teams.cloud.microsoft, personal account on teams.live.com and
 * teams.microsoft.com/light-meetings) and falls back to text or ARIA labels where that is all
 * there is. Readers return plain values and never log page content.
 */
import { CaptionSnapshot, TranscriptPanelItem } from "./captions";

/** Buttons shown only during a call. */
const IN_CALL_SELECTORS = [
  '[data-tid="hangup-main-btn"]',
  '[data-tid="call-hangup"]',
  "#hangup-button",
  '[data-tid="call-duration"]',
];

/** Tab-title parts that name a Teams section rather than the meeting. */
const SECTION_TITLES = new Set([
  "microsoft teams",
  "calendar",
  "chat",
  "activity",
  "teams",
  "calls",
  "meet",
  "onedrive",
  "people",
  "apps",
]);

/**
 * Whether the page is in a call. Inputs: The document. Side effects: None.
 */
export function isInCall(root: ParentNode): boolean {
  return IN_CALL_SELECTORS.some((selector) => root.querySelector(selector) !== null);
}

/**
 * Whether Teams transcription is running in the call, whoever started it.
 * Inputs: The document. Side effects: None.
 */
export function isTeamsTranscriptRunning(root: ParentNode): boolean {
  return (
    root.querySelector('[data-tid="call-transcript-indicator"], [data-tid="call-transcript-started"]') !==
    null
  );
}

/**
 * Takes the meeting title out of a tab title such as `(3) Calendar | Weekly sync | Microsoft Teams`.
 *
 * Inputs: `document.title`. Outputs: The title, or "" when the tab shows none. Side effects: None.
 */
export function meetingTitleFromDocumentTitle(documentTitle: string): string {
  const parts = documentTitle
    .replace(/^\(\d+\)\s*/, "")
    .split("|")
    .map((part) => part.trim())
    .filter((part) => part !== "" && !SECTION_TITLES.has(part.toLowerCase()));
  // Teams appends "(External)" or "(Guest)" to chats with people outside the organisation.
  return (parts[0] ?? "").replace(/\s*\((external|guest)\)$/i, "");
}

/** The visible text of an element (jsdom has no `innerText`). */
function textOf(element: Element | null | undefined): string {
  return (element?.textContent ?? "").replace(/\s+/g, " ").trim();
}

/** Leaf elements (no element children) with text, in document order. */
function textLeaves(container: Element): Element[] {
  return [...container.querySelectorAll("*")].filter(
    (element) => element.children.length === 0 && textOf(element) !== "",
  );
}

/**
 * Reads the captions on screen.
 *
 * Inputs: The document. Outputs: One snapshot per caption, keyed by its item element, with the
 * speaker's name and (work accounts) Teams ID. Side effects: None.
 */
export function readCaptions(root: ParentNode): CaptionSnapshot[] {
  const snapshots: CaptionSnapshot[] = [];
  for (const captionText of root.querySelectorAll('[data-tid="closed-caption-text"]')) {
    let item: Element = captionText;
    while (
      item.parentElement &&
      item.parentElement.querySelectorAll('[data-tid="closed-caption-text"]').length === 1
    ) {
      item = item.parentElement;
    }
    const text = textOf(captionText);
    const speaker =
      textLeaves(item)
        .filter((leaf) => !captionText.contains(leaf) && leaf !== captionText)
        .map(textOf)[0] ?? "";
    const speakerId =
      item.closest("[data-person-mri]")?.getAttribute("data-person-mri") ??
      item.querySelector("[data-person-mri]")?.getAttribute("data-person-mri") ??
      undefined;
    snapshots.push({ key: item, speaker, ...(speakerId ? { speakerId } : {}), text });
  }
  return snapshots;
}

/**
 * Reads the Teams transcript panel, when it is open.
 *
 * Inputs: The document. Outputs: The speech items (status lines such as "… started
 * transcription" have no author and are skipped). Side effects: None.
 */
export function readTranscriptPanel(root: ParentNode): TranscriptPanelItem[] {
  const items: TranscriptPanelItem[] = [];
  for (const item of root.querySelectorAll('[data-tid^="transcript-chat-message-"]')) {
    const id = (item.getAttribute("data-tid") ?? "").replace("transcript-chat-message-", "");
    const speaker = textOf(item.querySelector('[data-tid="transcript-item-author"]'));
    if (speaker === "") continue;
    const messages = [...item.querySelectorAll('[data-tid="call-transcript-panel-message"]')];
    const text = messages.length > 0 ? messages.map(textOf).join(" ") : "";
    if (text === "") continue;
    items.push({ id, speaker, text });
  }
  return items;
}

/** A recap card Teams posted in a meeting chat. */
export interface RecapCard {
  /** The card element. */
  element: Element;
  /** The meeting title on the card. */
  title: string;
  /** The meeting's start and end (local time), when the card shows them. */
  startMs: number | null;
  endMs: number | null;
  /** Whether the card lists a transcript. */
  hasTranscript: boolean;
  /** The "View recap" button. */
  viewRecapButton: HTMLElement;
}

/** English month names, for the recap card's date. */
const MONTHS = [
  "january",
  "february",
  "march",
  "april",
  "may",
  "june",
  "july",
  "august",
  "september",
  "october",
  "november",
  "december",
];

/**
 * Parses a recap card's `Monday, November 9, 2026 4:10 PM - 4:21 PM`.
 *
 * Inputs: The card text. Outputs: Start and end in local time, or `null`. Side effects: None.
 */
export function parseRecapTimeRange(text: string): { startMs: number; endMs: number } | null {
  const match =
    /([A-Za-z]+) (\d{1,2}), (\d{4}),? (\d{1,2}):(\d{2})\s*(AM|PM)\s*[-–]\s*(\d{1,2}):(\d{2})\s*(AM|PM)/i.exec(
      text,
    );
  if (!match) return null;
  const [, monthName, day, year, startHour, startMinute, startHalf, endHour, endMinute, endHalf] = match;
  const month = MONTHS.indexOf((monthName ?? "").toLowerCase());
  if (month < 0) return null;
  const hour24 = (hour: string | undefined, half: string | undefined) =>
    (Number(hour) % 12) + ((half ?? "").toUpperCase() === "PM" ? 12 : 0);
  const at = (hour: number, minute: string | undefined) =>
    new Date(Number(year), month, Number(day), hour, Number(minute)).getTime();
  const startMs = at(hour24(startHour, startHalf), startMinute);
  let endMs = at(hour24(endHour, endHalf), endMinute);
  if (endMs < startMs) endMs += 24 * 60 * 60 * 1000;
  return { startMs, endMs };
}

/**
 * Finds the recap cards in a meeting chat.
 *
 * Inputs: The document. Outputs: Each card with a "View recap" button. Side effects: None.
 */
export function findRecapCards(root: ParentNode): RecapCard[] {
  const cards: RecapCard[] = [];
  const buttons = [...root.querySelectorAll<HTMLElement>("button, [role='button']")].filter(
    (button) => /^view recap$/i.test(textOf(button)) || /^view recap$/i.test(button.getAttribute("aria-label") ?? ""),
  );
  for (const viewRecapButton of buttons) {
    let card: Element = viewRecapButton;
    while (card.parentElement && parseRecapTimeRange(textOf(card)) === null) card = card.parentElement;
    const times = parseRecapTimeRange(textOf(card));
    const leaves = textLeaves(card).map(textOf);
    const dateIndex = leaves.findIndex((leaf) => parseRecapTimeRange(leaf) !== null);
    const title = (dateIndex > 0 ? leaves[dateIndex - 1] : leaves[0]) ?? "";
    const hasTranscript = leaves.some((leaf) => /^transcript$/i.test(leaf));
    cards.push({
      element: card,
      title,
      startMs: times?.startMs ?? null,
      endMs: times?.endMs ?? null,
      hasTranscript,
      viewRecapButton,
    });
  }
  return cards;
}

/**
 * Finds the video showing a screen share, when one is on.
 *
 * Inputs: The document. Outputs: The playing share video, or `null`. Teams names a share's stream
 * `applicationsharingVideo-…`; when the stream is not readable, a large video outside the owner's
 * own tile is taken instead. Side effects: None.
 */
export function findShareVideo(root: ParentNode): HTMLVideoElement | null {
  const videos = [...root.querySelectorAll("video")];
  const byStream = videos.find((video) => {
    const stream = video.srcObject;
    return stream instanceof MediaStream && stream.id.startsWith("applicationsharingVideo");
  });
  if (byStream) return byStream;
  return (
    videos.find(
      (video) =>
        video.videoWidth >= 800 &&
        !video.closest('[data-tid="myself-video"]') &&
        video.closest('[data-tid="calling-stream"], [data-tid*="screen-share"], [data-tid="Stage-wrapper"]') !== null,
    ) ?? null
  );
}

/**
 * The owner's display name as Teams shows it, for the microphone track.
 *
 * Inputs: The document. Outputs: The name, or "Me". Side effects: None.
 */
export function ownerName(root: ParentNode): string {
  const picture = root.querySelector('[data-tid="me-control-avatar"]')?.getAttribute("aria-label") ?? "";
  const fromPicture = /^profile picture of (.+?)\.?$/i.exec(picture.trim())?.[1]?.trim();
  if (fromPicture) return fromPicture;
  const label = root.querySelector('[data-tid="me-control-avatar-trigger"]')?.getAttribute("aria-label") ?? "";
  // "Your profile, status …" names nobody.
  if (/^your profile\b/i.test(label.trim())) return "Me";
  const name = label.replace(/^(profile|account manager)( for)?,?\s*/i, "").split(",")[0]?.trim() ?? "";
  return name !== "" ? name : "Me";
}
