/**
 * Clicks through the Teams interface on the owner's behalf (Issue #102): turning on live
 * captions, starting Teams transcription (only when the owner opted in), and opening a meeting's
 * recap to fetch its transcript.
 *
 * Teams has no API for these in the browser, so each step finds a control by its `data-tid`,
 * ARIA label or English text, clicks it, and waits for the next one. A step that does not find
 * its control gives up quietly (closing any menu it opened) and reports `false`; nothing here
 * changes the meeting otherwise. Page text is never logged.
 */

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

/** The visible label of a control: its ARIA label or its text. */
function labelOf(element: Element): string {
  return (element.getAttribute("aria-label") ?? element.textContent ?? "").replace(/\s+/g, " ").trim();
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
    control.dispatchEvent(new MouseEvent(type, { bubbles: true, cancelable: true, view: window }));
  }
  control.click();
}

/** Waits for a control and presses it. Outputs: Whether it was found. */
async function pressWhenShown(root: ParentNode, label: RegExp, timeoutMs = STEP_TIMEOUT_MS): Promise<boolean> {
  const control = await waitFor(() => findControl(root, label), timeoutMs);
  if (!control) return false;
  press(control);
  return true;
}

/** Closes an open menu. */
function closeMenus(): void {
  document.activeElement?.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
  document.body.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape", bubbles: true }));
}

/** Opens the call toolbar's "More" menu. Outputs: Whether it opened. */
async function openMoreMenu(root: ParentNode): Promise<boolean> {
  const more =
    root.querySelector<HTMLElement>('#callingButtons-showMoreBtn, [data-tid="more-button"], [data-tid="callingButtons-showMoreBtn"]') ??
    findControl(root, /^more( actions| options)?$/i);
  if (!more) return false;
  press(more);
  return true;
}

/**
 * Turns on live captions when they are off.
 *
 * Inputs: The document. Outputs: Whether captions are on afterwards.
 * Side effects: Opens the More menu and clicks the captions item (work accounts: Language and
 * speech → Show live captions; personal accounts: Captions).
 */
export async function turnOnCaptions(root: ParentNode): Promise<boolean> {
  const captionsShown = () =>
    root.querySelector('[data-tid="closed-caption-renderer-wrapper"], [data-tid="closed-caption-v2-window-wrapper"]');
  if (captionsShown()) return true;
  if (!(await openMoreMenu(root))) return false;
  const direct = await waitFor(() => findControl(root, /^(show |turn on )?(live )?captions$/i), 1_500);
  if (direct) {
    press(direct);
  } else if (await pressWhenShown(root, /^language and speech$/i)) {
    if (!(await pressWhenShown(root, /^(show|turn on) live captions$/i))) {
      closeMenus();
      return false;
    }
  } else {
    closeMenus();
    return false;
  }
  return (await waitFor(captionsShown, 5_000)) !== null;
}

/**
 * Starts Teams transcription (the owner's opt-in), when the meeting allows it.
 *
 * Inputs: The document. Outputs: Whether transcription was started.
 * Side effects: Opens More → Record and transcribe → Start transcription and confirms the
 * spoken-language prompt. Everyone in the meeting sees that transcription started.
 */
export async function startTeamsTranscription(root: ParentNode): Promise<boolean> {
  if (!(await openMoreMenu(root))) return false;
  if (!(await pressWhenShown(root, /^record and transcribe$/i))) {
    closeMenus();
    return false;
  }
  if (!(await pressWhenShown(root, /^start transcription$/i))) {
    closeMenus();
    return false;
  }
  await pressWhenShown(root, /^confirm$/i, 3_000);
  return (await waitFor(() => root.querySelector('[data-tid="call-transcript-indicator"]'), 8_000)) !== null;
}

/**
 * Opens a meeting's recap and asks Teams for its transcript.
 *
 * Inputs: The card's "View recap" button, the document, and a wait for the transcript file the
 * page hook copies (true once one arrived).
 * Outputs: Whether a transcript arrived. First the recap's Transcript tab is opened (Teams may
 * load the file to show it); if no file arrives, Download → "Download as .vtt" is used, which
 * also saves a copy in the browser's downloads.
 * Side effects: Navigates within Teams; may download a file.
 */
export async function fetchRecapTranscript(
  viewRecapButton: HTMLElement,
  root: ParentNode,
  transcriptArrived: (timeoutMs: number) => Promise<boolean>,
): Promise<boolean> {
  press(viewRecapButton);
  if (!(await pressWhenShown(root, /^transcript$/i, 8_000))) return false;
  if (await transcriptArrived(6_000)) return true;
  if (!(await pressWhenShown(root, /^download$/i))) return false;
  if (!(await pressWhenShown(root, /download as \.?vtt/i))) {
    closeMenus();
    return false;
  }
  return transcriptArrived(10_000);
}
