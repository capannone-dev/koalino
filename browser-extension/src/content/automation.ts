/**
 * Clicks through the Teams interface on the owner's behalf (Issue #102): turning on live
 * captions, starting Teams transcription (only when the owner opted in), and opening a meeting's
 * recap to fetch its transcript.
 *
 * Teams has no API for these in the browser, so each step finds a control by its element ID (the
 * ones the #102 live test found), else by ARIA label or English text, clicks it, and waits for the
 * next one. Steps run one after another, so two never fight over the same menu. A step that does
 * not find its control gives up quietly (closing the menu it opened) and reports `false`; nothing
 * here changes the meeting otherwise. Page text is never logged.
 */
import { closestSession } from "../lib/meetings";
import { parseRecapTimeRange, withoutDoubledLabel } from "../lib/teamsDom";

/** How long a step waits for the next control. */
const STEP_TIMEOUT_MS = 4_000;
/** How often a wait re-checks. */
const POLL_MS = 150;

/**
 * Waits until `find` returns something.
 *
 * Inputs: The finder and the longest wait. Outputs: Its result, or `null` on timeout.
 * Side effects: None besides waiting.
 */
export async function waitFor<T>(find: () => T | null, timeoutMs = STEP_TIMEOUT_MS): Promise<T | null> {
  const deadline = Date.now() + timeoutMs;
  for (;;) {
    const found = find();
    if (found) return found;
    if (Date.now() >= deadline) return null;
    await new Promise((resolve) => setTimeout(resolve, POLL_MS));
  }
}

/** A keyboard shortcut Teams shows after a menu item's label, such as "Alt+Shift+C". */
const TRAILING_SHORTCUT = /\s*(ctrl|alt|shift|cmd|option|⌘|⌥)\s*\+.*$/i;

/**
 * The visible label of a control: its ARIA label or its text, read once (Fluent tabs double it),
 * without a trailing shortcut.
 */
function labelOf(element: Element): string {
  const label = (element.getAttribute("aria-label") ?? element.textContent ?? "").replace(/\s+/g, " ").trim();
  return withoutDoubledLabel(label).replace(TRAILING_SHORTCUT, "");
}

/**
 * Finds a clickable control (button, menu item, tab, link) whose label matches.
 *
 * Inputs: Where to look and the label pattern. Outputs: The control, or `null`. Side effects: None.
 */
export function findControl(root: ParentNode, label: RegExp): HTMLElement | null {
  const candidates = root.querySelectorAll<HTMLElement>(
    "button, [role='button'], [role='menuitem'], [role='menuitemcheckbox'], [role='menuitemradio'], [role='tab'], a",
  );
  for (const candidate of candidates) {
    if (candidate.getAttribute("aria-disabled") === "true" || (candidate as HTMLButtonElement).disabled) continue;
    if (label.test(labelOf(candidate))) return candidate;
  }
  return null;
}

/** Clicks a control the way a person would (pointer events, then click). */
function press(control: HTMLElement): void {
  for (const type of ["pointerdown", "mousedown", "pointerup", "mouseup"]) {
    control.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true }));
  }
  control.click();
}

/** A control to find: its element ID when Teams gives one, and its label otherwise. */
interface ControlSpec {
  id?: string;
  label: RegExp;
}

/** Finds a control by ID, then by label. */
function findSpec(root: ParentNode, spec: ControlSpec): HTMLElement | null {
  const byId = spec.id ? root.querySelector<HTMLElement>(`#${spec.id}`) : null;
  return byId ?? findControl(root, spec.label);
}

/** Waits for a control and presses it. Outputs: Whether it was found. */
async function pressWhenShown(root: ParentNode, spec: ControlSpec, timeoutMs = STEP_TIMEOUT_MS): Promise<boolean> {
  const control = await waitFor(() => findSpec(root, spec), timeoutMs);
  if (!control) return false;
  press(control);
  return true;
}

/** The call toolbar's "More" button. */
function moreButton(root: ParentNode): HTMLElement | null {
  return (
    root.querySelector<HTMLElement>(
      '#callingButtons-showMoreBtn, [data-tid="more-button"], [data-tid="callingButtons-showMoreBtn"]',
    ) ?? findControl(root, /^more( actions| options)?$/i)
  );
}

/**
 * Closes the "More" menu by pressing its button again (Teams ignores a scripted Escape), or
 * sends Escape when there is no such button.
 */
function closeMenus(root: ParentNode): void {
  const more = moreButton(root);
  if (more?.getAttribute("aria-expanded") === "true") {
    press(more);
    return;
  }
  document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
}

/** Opens the call toolbar's "More" menu, leaving it open when it already is. Outputs: Whether it is open. */
async function openMoreMenu(root: ParentNode): Promise<boolean> {
  const more = moreButton(root);
  if (!more) return false;
  if (more.getAttribute("aria-expanded") !== "true") press(more);
  return true;
}

/** The automation step running now; the next one starts after it. */
let running: Promise<unknown> = Promise.resolve();

/**
 * Runs one step after any step already running.
 *
 * Inputs: The step. Outputs: Its result (`false` when it threw). Side effects: Those of the step.
 */
function oneAtATime(step: () => Promise<boolean>): Promise<boolean> {
  const result = running.then(step, step).catch(() => false);
  running = result;
  return result;
}

const LANGUAGE_AND_SPEECH: ControlSpec = { id: "LanguageSpeechMenuControl-id", label: /^language and speech$/i };
const SHOW_CAPTIONS: ControlSpec = { id: "closed-captions-button", label: /^(show |turn on )?(live )?captions$/i };
const RECORD_AND_TRANSCRIBE: ControlSpec = { id: "RecordingMenuControl-id", label: /^record and transcribe$/i };
const START_TRANSCRIPTION: ControlSpec = { id: "call-transcript-button", label: /^start transcription$/i };

/**
 * Turns on live captions when they are off.
 *
 * Inputs: The document. Outputs: Whether captions are on afterwards.
 * Side effects: Opens the More menu and clicks the captions item (work accounts: Language and
 * speech → Show live captions; personal accounts: Captions).
 */
export function turnOnCaptions(root: ParentNode): Promise<boolean> {
  return oneAtATime(async () => {
    const captionsShown = () =>
      root.querySelector('[data-tid="closed-caption-renderer-wrapper"], [data-tid="closed-caption-v2-window-wrapper"]');
    if (captionsShown()) return true;
    if (!(await openMoreMenu(root))) return false;
    // Personal accounts list Captions in the menu itself; work accounts under Language and speech.
    const direct = await waitFor(() => findSpec(root, SHOW_CAPTIONS), 1_500);
    if (direct) {
      press(direct);
    } else if (!(await pressWhenShown(root, LANGUAGE_AND_SPEECH)) || !(await pressWhenShown(root, SHOW_CAPTIONS))) {
      closeMenus(root);
      return false;
    }
    const shown = (await waitFor(captionsShown, 5_000)) !== null;
    closeMenus(root);
    return shown;
  });
}

/**
 * Starts Teams transcription (the owner's opt-in), when the meeting allows it.
 *
 * Inputs: The document. Outputs: Whether transcription was started.
 * Side effects: Opens More → Record and transcribe → Start transcription and confirms the
 * spoken-language prompt. Everyone in the meeting sees that transcription started.
 */
export function startTeamsTranscription(root: ParentNode): Promise<boolean> {
  return oneAtATime(async () => {
    if (!(await openMoreMenu(root))) return false;
    if (!(await pressWhenShown(root, RECORD_AND_TRANSCRIBE)) || !(await pressWhenShown(root, START_TRANSCRIPTION))) {
      closeMenus(root);
      return false;
    }
    await pressWhenShown(root, { label: /^confirm$/i }, 3_000);
    const started = (await waitFor(() => root.querySelector('[data-tid="call-transcript-indicator"]'), 8_000)) !== null;
    closeMenus(root);
    return started;
  });
}

const SPOKEN_LANGUAGE: ControlSpec = { label: /^(change )?(meeting )?spoken language|^language settings$/i };
const SPOKEN_LANGUAGE_DROPDOWN = /spoken language/i;
const APPLY_LANGUAGE: ControlSpec = { label: /^(update|confirm|save|apply)$/i };
const CANCEL: ControlSpec = { label: /^cancel$/i };

/** How setting Teams' spoken language ended. */
export type SpokenLanguageOutcome = "set" | "already" | "failed";

/**
 * The names Teams may give a language: in the page's language and in English ("Italian").
 *
 * Inputs: A Whisper language code ("it"). Outputs: Lower-case names, possibly empty.
 * Side effects: None.
 */
function languageNames(code: string): string[] {
  const names = new Set<string>();
  for (const locale of [document.documentElement.lang, navigator.language, "en"]) {
    if (!locale) continue;
    try {
      const name = new Intl.DisplayNames([locale], { type: "language" }).of(code);
      if (name && name.toLowerCase() !== code) names.add(name.toLowerCase());
    } catch {
      // An unknown locale or code names nothing.
    }
  }
  return [...names];
}

/** Whether a Teams language label ("Italian (Italy)") is one of `names`. */
function namesLanguage(label: string, names: string[]): boolean {
  const language = label.replace(/\s*\(.*$/, "").trim().toLowerCase();
  return names.includes(language);
}

/** The dialog's spoken-language dropdown. */
function spokenLanguageDropdown(root: ParentNode): HTMLElement | null {
  for (const candidate of root.querySelectorAll<HTMLElement>("[role='combobox'], select, button[aria-haspopup]")) {
    const labelledBy = candidate.getAttribute("aria-labelledby");
    const label = labelledBy ? document.getElementById(labelledBy)?.textContent ?? "" : candidate.getAttribute("aria-label") ?? "";
    if (SPOKEN_LANGUAGE_DROPDOWN.test(label)) return candidate;
  }
  return null;
}

const CAPTIONS_SETTINGS_BUTTON = '[data-tid="closed-captions-settings-menu-trigger-button"]';
const CAPTIONS_SPOKEN_LANGUAGE_ITEM = '[data-tid="captions-settings-redesign-spoken-language"]';
/** Prefix of each language item's `data-tid`; a locale such as `it-it` follows it. */
const CAPTIONS_SPOKEN_LANGUAGE_OPTION_PREFIX = "captions-settings-redesign-spoken-language-";

/**
 * The captions settings' language item for `code`: by the locale in its `data-tid` (preferring
 * the language's own country, `it-it` before `it-ch`), else by its name.
 *
 * Inputs: The document, a Whisper code and the language's names. Outputs: The item, or `null`.
 * Side effects: None.
 */
function captionsLanguageOption(root: ParentNode, code: string, names: string[]): HTMLElement | null {
  const options = [
    ...root.querySelectorAll<HTMLElement>(`[data-tid^="${CAPTIONS_SPOKEN_LANGUAGE_OPTION_PREFIX}"]`),
  ];
  const localeOf = (option: HTMLElement) =>
    (option.getAttribute("data-tid") ?? "").slice(CAPTIONS_SPOKEN_LANGUAGE_OPTION_PREFIX.length).toLowerCase();
  const byCode = options.filter((option) => localeOf(option).split("-")[0] === code.toLowerCase());
  return (
    byCode.find((option) => localeOf(option) === `${code}-${code}`.toLowerCase()) ??
    byCode[0] ??
    options.find((option) => namesLanguage(labelOf(option), names)) ??
    null
  );
}

/**
 * Sets the spoken language from the live captions' settings menu, where Teams keeps it since
 * late 2026: the captions gear → Meeting spoken language → a list of languages that applies the
 * chosen one at once.
 *
 * Inputs: The document, a Whisper code and the language's names.
 * Outputs: The outcome, or `null` when the captions settings are not shown (captions off, or an
 * older Teams), so the caller tries the More menu instead.
 * Side effects: Opens and closes the captions settings menu; may change the meeting's language.
 */
async function setSpokenLanguageFromCaptions(
  root: ParentNode,
  code: string,
  names: string[],
): Promise<SpokenLanguageOutcome | null> {
  const gear = root.querySelector<HTMLElement>(CAPTIONS_SETTINGS_BUTTON);
  if (!gear) return null;
  const closeSettings = () => {
    if (gear.getAttribute("aria-expanded") === "true") press(gear);
  };
  if (gear.getAttribute("aria-expanded") !== "true") press(gear);
  const spoken = await waitFor(() => root.querySelector<HTMLElement>(CAPTIONS_SPOKEN_LANGUAGE_ITEM));
  if (!spoken) {
    closeSettings();
    return "failed";
  }
  // The item reads "Meeting spoken language" then the current language, in its last element.
  const current = (spoken.lastElementChild?.textContent ?? spoken.textContent ?? "").replace(/\s+/g, " ").trim();
  if (namesLanguage(current, names)) {
    closeSettings();
    return "already";
  }
  press(spoken);
  const option = await waitFor(() => captionsLanguageOption(root, code, names), 2_000);
  if (!option) {
    closeSettings();
    return "failed";
  }
  if (option.getAttribute("aria-checked") === "true") {
    closeSettings();
    return "already";
  }
  press(option);
  // Some tenants confirm the change in a dialog; press its button when one shows.
  const confirm = await waitFor(() => {
    for (const dialog of root.querySelectorAll<HTMLElement>("[role='dialog'], [role='alertdialog']")) {
      const button = findControl(dialog, APPLY_LANGUAGE.label);
      if (button) return button;
    }
    return null;
  }, 1_500);
  if (confirm) press(confirm);
  closeSettings();
  return "set";
}

/**
 * Sets the meeting's spoken language in Teams to Koalino's (#141; the owner's opt-in).
 *
 * Inputs: The document and a Whisper language code. Outputs: `set`, `already` when Teams used
 * it already, or `failed` (control or language not found; the menu or dialog is closed).
 * Side effects: Uses the live captions' settings (gear → Meeting spoken language) when captions
 * are shown, else More → Language and speech → Change spoken language and Update, as older Teams
 * versions have it. Teams applies it to the whole meeting and may tell the participants.
 */
export function setTeamsSpokenLanguage(root: ParentNode, code: string): Promise<SpokenLanguageOutcome> {
  let outcome: SpokenLanguageOutcome = "failed";
  return oneAtATime(async () => {
    const names = languageNames(code);
    if (names.length === 0) return false;
    const fromCaptions = await setSpokenLanguageFromCaptions(root, code, names);
    if (fromCaptions) {
      outcome = fromCaptions;
      return fromCaptions !== "failed";
    }
    if (!(await openMoreMenu(root))) return false;
    const direct = await waitFor(() => findSpec(root, SPOKEN_LANGUAGE), 1_500);
    if (direct) {
      press(direct);
    } else if (!(await pressWhenShown(root, LANGUAGE_AND_SPEECH)) || !(await pressWhenShown(root, SPOKEN_LANGUAGE))) {
      closeMenus(root);
      return false;
    }
    const dropdown = await waitFor(() => spokenLanguageDropdown(root));
    if (!dropdown) {
      closeMenus(root);
      return false;
    }
    const cancel = () => {
      const button = findSpec(root, CANCEL);
      if (button) press(button);
      closeMenus(root);
    };
    // The dropdown's label names the field; its text or selected option is the current language.
    const shown =
      dropdown instanceof HTMLSelectElement
        ? (dropdown.selectedOptions[0]?.textContent ?? "")
        : (dropdown.getAttribute("value") ?? dropdown.textContent ?? "");
    if (namesLanguage(shown.replace(/\s+/g, " "), names)) {
      outcome = "already";
      cancel();
      return true;
    }
    press(dropdown);
    const option = await waitFor(() => {
      for (const candidate of root.querySelectorAll<HTMLElement>("[role='option'], option")) {
        if (namesLanguage(labelOf(candidate), names)) return candidate;
      }
      return null;
    }, 2_000);
    if (!option) {
      press(dropdown);
      cancel();
      return false;
    }
    press(option);
    if (!(await pressWhenShown(root, APPLY_LANGUAGE))) {
      cancel();
      return false;
    }
    outcome = "set";
    closeMenus(root);
    return true;
  }).then(() => outcome);
}

/** How often the recap frame is asked to download the transcript before giving up. */
const RECAP_DOWNLOAD_ATTEMPTS = 3;

/** How fetching a recap transcript ended: sent, or the step that failed. */
export type RecapFetchOutcome = "sent" | "no-transcript-tab" | "wrong-session" | "nothing-arrived";

/** The recap view's session picker ("Select the meeting by time"), when shown. */
function sessionPicker(root: ParentNode): HTMLElement | null {
  return (
    [...root.querySelectorAll<HTMLElement>('[role="combobox"]')].find((element) =>
      /select the meeting by time/i.test(element.getAttribute("aria-label") ?? ""),
    ) ?? null
  );
}

/** The session times a picker or option shows, or nulls when it shows none. */
function sessionTimes(element: Element | null): { startMs: number | null; endMs: number | null } {
  const times = element ? parseRecapTimeRange((element.textContent ?? "").replace(/\s+/g, " ")) : null;
  return { startMs: times?.startMs ?? null, endMs: times?.endMs ?? null };
}

/**
 * Makes the recap view show the recording's session (#158).
 *
 * A meeting chat keeps one recap per session and Teams may open any of them, so the session shown
 * decides whose transcript a download yields.
 *
 * Inputs: The Teams document, the recording's start and end, and how long to wait for each step.
 * Outputs: `true` when the view shows the session nearest the recording (`closestSession`), or
 * shows no session picker at all (a chat with one session); `false` when no listed session fits.
 * Side effects: May open the picker and choose a session.
 */
async function showRecordedSession(
  root: ParentNode,
  recording: { startMs: number; endMs: number },
  timeoutMs: number,
): Promise<boolean> {
  const picker = sessionPicker(root);
  if (!picker) return true;
  const shown = sessionTimes(picker);
  // A session the recording overlaps is right; anything else is checked against the list.
  const shownEnd = shown.endMs ?? shown.startMs;
  if (shown.startMs !== null && shownEnd !== null && shown.startMs <= recording.endMs && shownEnd >= recording.startMs) {
    return true;
  }
  press(picker);
  const options = await waitFor(() => {
    const found = [...root.querySelectorAll<HTMLElement>('[role="option"]')];
    return found.length > 0 ? found : null;
  }, timeoutMs);
  const best = closestSession(
    (options ?? []).map((option) => ({ option, ...sessionTimes(option) })),
    recording,
  );
  if (!best) {
    if (options) press(picker);
    return false;
  }
  press(best.option);
  return Boolean(await waitFor(() => (sessionTimes(sessionPicker(root)).startMs === best.startMs ? true : null), timeoutMs));
}

/**
 * Opens a meeting's recap transcript and has the recap frame download it.
 *
 * Teams shows the transcript in a SharePoint frame whose own copy is encrypted for its player.
 * Only Download → "Download as .vtt" there yields WebVTT, which the page hook in that frame copies
 * (the browser also keeps the file in its downloads).
 *
 * A meeting chat keeps one recap per session, and the view may open on another session than the
 * recorded one, so the session shown is checked first (#158).
 *
 * Inputs: The Teams document; a recap card's "View recap" button, or `null` when the chat already
 * shows the recap; the recording's start and end (`null` skips the session check);
 * `requestFrameDownload`, which asks the recap frame to run `downloadRecapVtt`; and a wait for the
 * copied transcript (true once one arrived). `tabTimeoutMs` is how long to wait for the
 * Transcript tab.
 * Outputs: `sent` once a transcript arrived, otherwise the step that failed (`wrong-session` when
 * no session listed fits the recording; nothing was downloaded then).
 * Side effects: Navigates within Teams (may choose another session); downloads a file.
 */
export async function fetchRecapTranscript(
  root: ParentNode,
  viewRecapButton: HTMLElement | null,
  recording: { startMs: number; endMs: number } | null,
  requestFrameDownload: () => void,
  transcriptArrived: (timeoutMs: number) => Promise<boolean>,
  tabTimeoutMs = 20_000,
): Promise<RecapFetchOutcome> {
  if (viewRecapButton) press(viewRecapButton);
  const transcriptTab = { label: /^transcript$/i };
  if (!(await pressWhenShown(root, transcriptTab, tabTimeoutMs))) return "no-transcript-tab";
  if (recording) {
    if (!(await showRecordedSession(root, recording, STEP_TIMEOUT_MS))) return "wrong-session";
    // Choosing a session reloads the recap: open its Transcript tab again.
    if (!(await pressWhenShown(root, transcriptTab, tabTimeoutMs))) return "no-transcript-tab";
  }
  // The frame may still be loading: ask again until a transcript arrives.
  for (let attempt = 0; attempt < RECAP_DOWNLOAD_ATTEMPTS; attempt += 1) {
    requestFrameDownload();
    if (await transcriptArrived(8_000)) return "sent";
  }
  return "nothing-arrived";
}

/**
 * In the recap's SharePoint frame: presses Download, then "Download as .vtt".
 *
 * Inputs: The frame's document and how long to wait for each control.
 * Outputs: Whether both were pressed. Side effects: The page downloads the transcript.
 */
export async function downloadRecapVtt(root: ParentNode, timeoutMs = 10_000): Promise<boolean> {
  if (!(await pressWhenShown(root, { label: /^download$/i }, timeoutMs))) return false;
  if (await pressWhenShown(root, { label: /download as \.?vtt/i })) return true;
  closeMenus(root);
  return false;
}
