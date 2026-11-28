/**
 * The extension's small overlay on a call page (Issue #102; any site since #174): the consent
 * question when a call starts ("Record this meeting with Koalino?") and, while recording, a
 * visible indicator with a Stop button. It lives in a closed shadow root so the page's styles and
 * scripts do not reach it.
 */
import { msg, textDirection } from "../lib/i18n";

/** What the owner answered. */
export type ConsentAnswer = "record" | "always" | "not-now" | "never";

/**
 * The overlay's styles: a dark panel over any page, with the main button in the dark scheme's
 * accent from branding/tokens.json (#206).
 */
const STYLE = `
  :host { all: initial; }
  .panel { position: fixed; top: 12px; left: 50%; transform: translateX(-50%); z-index: 2147483647;
    font: 14px/1.4 system-ui, sans-serif; color: #fff; background: #1f2937; border-radius: 10px;
    box-shadow: 0 6px 24px rgba(0,0,0,.35); padding: 10px 14px; display: flex; gap: 10px; align-items: center; }
  .dot { width: 10px; height: 10px; border-radius: 50%; background: #ef4444; flex: none; }
  button { font: inherit; border: 0; border-radius: 6px; padding: 5px 10px; cursor: pointer;
    background: #374151; color: #fff; }
  button.primary { background: #8f45b8; }
  button:focus-visible { outline: 2px solid #c48ee0; outline-offset: 2px; }
  .detail { opacity: .8; font-size: 12px; }
`;

/** The overlay element and its shadow root. */
let host: HTMLElement | null = null;
let shadow: ShadowRoot | null = null;

/** Clears and returns the panel, creating the overlay on first use. */
function panel(): HTMLDivElement {
  if (!host || !host.isConnected) {
    host = document.createElement("note-taker-overlay");
    shadow = host.attachShadow({ mode: "closed" });
    document.documentElement.append(host);
  }
  const root = shadow as ShadowRoot;
  root.replaceChildren();
  const style = document.createElement("style");
  style.textContent = STYLE;
  const container = document.createElement("div");
  container.className = "panel";
  container.dir = textDirection();
  root.append(style, container);
  return container;
}

/** A button with a label. */
function button(label: string, onClick: () => void, primary = false): HTMLButtonElement {
  const element = document.createElement("button");
  element.textContent = label;
  if (primary) element.className = "primary";
  element.addEventListener("click", onClick);
  return element;
}

/**
 * Asks whether to record the meeting.
 *
 * Inputs: The answer's receiver; the site's name for a call elsewhere than Teams (#174), or null
 * on Teams. Side effects: Shows the overlay until answered or hidden.
 */
export function askConsent(onAnswer: (answer: ConsentAnswer) => void, site: string | null = null): void {
  const container = panel();
  const text = document.createElement("span");
  text.textContent = site === null ? msg("recordQuestion") : msg("recordCallQuestion", { site });
  const answer = (value: ConsentAnswer) => () => {
    hideOverlay();
    onAnswer(value);
  };
  container.append(
    text,
    button(msg("record"), answer("record"), true),
    button(site === null ? msg("alwaysForTeams") : msg("alwaysOnSite"), answer("always")),
    button(msg("notNow"), answer("not-now")),
  );
  if (site !== null) container.append(button(msg("neverOnSite"), answer("never")));
}

/**
 * Shows that recording is on.
 *
 * Inputs: A short status (for example whether Koalino is reachable) and the Stop handler.
 * Side effects: Shows the overlay.
 */
export function showRecording(status: string, onStop: () => void): void {
  const container = panel();
  const dot = document.createElement("span");
  dot.className = "dot";
  const text = document.createElement("span");
  text.textContent = msg("koalinoIsRecording");
  const detail = document.createElement("span");
  detail.className = "detail";
  detail.textContent = status;
  container.append(dot, text, detail, button(msg("stop"), onStop));
}

/** Removes the overlay. Side effects: Detaches it from the page. */
export function hideOverlay(): void {
  host?.remove();
  host = null;
  shadow = null;
}
