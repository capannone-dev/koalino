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

/**
 * A control's label read once. Teams' Fluent tabs hold their label twice when unselected (the
 * visible text and a hidden bold copy that reserves its width), so "TranscriptTranscript" is
 * "Transcript".
 *
 * Inputs: The label text. Outputs: Its first half when both halves are equal, else the text.
 * Side effects: None.
 */
export function withoutDoubledLabel(label: string): string {
  const half = label.length / 2;
  return Number.isInteger(half) && half > 0 && label.slice(0, half) === label.slice(half) ? label.slice(0, half) : label;
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
/** The recap a meeting chat opens on after a meeting (2026 Teams), when it has a transcript. */
export interface RecapView {
  /** The chat's title, which is the meeting's. */
  title: string;
  /** The selected session's start and end (local time), when shown. */
  startMs: number | null;
  endMs: number | null;
  /** The Transcript tab; the transcript itself is in a SharePoint frame below it. */
  transcriptTab: HTMLElement;
}

/**
 * Finds the recap view: the session picker ("Select the meeting by time") and a Transcript tab.
 *
 * Inputs: The document. Outputs: The view, or `null` when the chat shows none or no transcript.
 * Side effects: None.
 */
export function findRecapView(root: ParentNode): RecapView | null {
  const transcriptTab =
    root.querySelector<HTMLElement>('[role="tab"][data-tid="Transcript"]') ??
    [...root.querySelectorAll<HTMLElement>('[role="tab"]')].find((tab) =>
      /^transcript$/i.test(withoutDoubledLabel(textOf(tab))),
    ) ??
    null;
  if (!transcriptTab) return null;
  const picker = [...root.querySelectorAll('[role="combobox"]')].find((element) =>
    /select the meeting by time/i.test(element.getAttribute("aria-label") ?? ""),
  );
  const times = parseRecapTimeRange(textOf(picker));
  const titleElement = root.querySelector('[data-tid="chat-title"]');
  const title =
    (titleElement ? textLeaves(titleElement) : [])
      .map(textOf)
      .find((leaf) => !/^(external|guest)$/i.test(leaf)) ?? "";
  return { title, startMs: times?.startMs ?? null, endMs: times?.endMs ?? null, transcriptTab };
}

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

/** A whole recap card in a meeting chat (live test, November 2026). */
const RECAP_CARD = '[data-testid="meeting-recap-chiclet"]';

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
    // Teams' card (2026) puts the title, times and "View recap" in its top half and the
    // Transcript in its bottom half, so the whole card is taken, not the first block with times.
    let card: Element | null = viewRecapButton.closest(RECAP_CARD);
    if (!card) {
      card = viewRecapButton;
      while (card.parentElement && parseRecapTimeRange(textOf(card)) === null) card = card.parentElement;
    }
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

/** The stream id prefix Teams gives a screen share (cameras are `mainVideo-…` and others). */
const SHARE_STREAM_PREFIX = "applicationsharingVideo";

/**
 * Finds the video showing a screen share, when one is on.
 *
 * Inputs: The document. Outputs: The share video, or `null`. Only a video whose stream Teams
 * named `applicationsharingVideo-…` counts (#102 probe). Cameras are shown in the same stage
 * tiles, at the same sizes, so no guess by size or place is made: slides are never taken from a
 * camera (#137). Side effects: None.
 */
export function findShareVideo(root: ParentNode): HTMLVideoElement | null {
  const videos = [...root.querySelectorAll("video")];
  return (
    videos.find((video) => {
      const stream = video.srcObject as { id?: unknown } | null;
      return typeof stream?.id === "string" && stream.id.startsWith(SHARE_STREAM_PREFIX);
    }) ?? null
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
