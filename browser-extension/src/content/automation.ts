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
import { withoutDoubledLabel } from "../lib/teamsDom";

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

/**
 * Sets the meeting's spoken language in Teams to Koalino's (#141; the owner's opt-in).
 *
 * Inputs: The document and a Whisper language code. Outputs: `set`, `already` when Teams used
 * it already, or `failed` (control or language not found; the dialog is cancelled).
 * Side effects: Opens More → Language and speech → Change spoken language, chooses the language
 * and presses Update. Teams applies it to the whole meeting and may tell the participants.
 */
export function setTeamsSpokenLanguage(root: ParentNode, code: string): Promise<SpokenLanguageOutcome> {
  let outcome: SpokenLanguageOutcome = "failed";
  return oneAtATime(async () => {
    const names = languageNames(code);
    if (names.length === 0 || !(await openMoreMenu(root))) return false;
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
export type RecapFetchOutcome = "sent" | "no-transcript-tab" | "nothing-arrived";

/**
 * Opens a meeting's recap transcript and has the recap frame download it.
 *
 * Teams shows the transcript in a SharePoint frame whose own copy is encrypted for its player.
 * Only Download → "Download as .vtt" there yields WebVTT, which the page hook in that frame copies
 * (the browser also keeps the file in its downloads).
 *
 * Inputs: The Teams document; a recap card's "View recap" button, or `null` when the chat already
 * shows the recap; `requestFrameDownload`, which asks the recap frame to run `downloadRecapVtt`;
 * and a wait for the copied transcript (true once one arrived).
 * `tabTimeoutMs` is how long to wait for the Transcript tab.
 * Outputs: `sent` once a transcript arrived, otherwise the step that failed.
 * Side effects: Navigates within Teams; downloads a file.
 */
export async function fetchRecapTranscript(
  root: ParentNode,
  viewRecapButton: HTMLElement | null,
  requestFrameDownload: () => void,
  transcriptArrived: (timeoutMs: number) => Promise<boolean>,
  tabTimeoutMs = 20_000,
): Promise<RecapFetchOutcome> {
  if (viewRecapButton) press(viewRecapButton);
  if (!(await pressWhenShown(root, { label: /^transcript$/i }, tabTimeoutMs))) return "no-transcript-tab";
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
