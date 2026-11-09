/**
 * Specifies the Teams automation against a model of the call's "More" menu as the #102 live test
 * in Firefox showed it (work account, November 2026): the button toggles a menu, submenus open on
 * click, and items carry stable IDs and, for some, a keyboard shortcut after their label.
 */
import { beforeEach, describe, expect, it } from "vitest";
import { findControl, startTeamsTranscription, turnOnCaptions } from "../src/content/automation";

/** Builds a menu item that runs `onPress` when clicked. */
function item(id: string, role: string, html: string, onPress: () => void): HTMLElement {
  const element = document.createElement("div");
  element.id = id;
  element.setAttribute("role", role);
  element.innerHTML = html;
  element.addEventListener("click", onPress);
  return element;
}

/** Shows `items` as a popup menu replacing any open one. */
function showSubmenu(items: HTMLElement[]): void {
  document.getElementById("submenu")?.remove();
  const menu = document.createElement("div");
  menu.id = "submenu";
  menu.setAttribute("role", "menu");
  menu.append(...items);
  document.body.append(menu);
}

/** Builds the call toolbar's More button and its menu. */
function buildTeamsCall(): void {
  document.body.innerHTML = '<button id="callingButtons-showMoreBtn" aria-expanded="false">More</button>';
  const more = document.getElementById("callingButtons-showMoreBtn") as HTMLElement;
  more.addEventListener("click", () => {
    const open = more.getAttribute("aria-expanded") === "true";
    document.getElementById("more-menu")?.remove();
    document.getElementById("submenu")?.remove();
    more.setAttribute("aria-expanded", String(!open));
    if (open) return;
    const menu = document.createElement("div");
    menu.id = "more-menu";
    menu.setAttribute("role", "menu");
    menu.append(
      item("RecordingMenuControl-id", "menuitem", "Record and transcribe", () =>
        showSubmenu([
          item("recording-button", "menuitemcheckbox", "Start recording", () => undefined),
          item("call-transcript-button", "menuitemcheckbox", "Start transcription", () => {
            const indicator = document.createElement("div");
            indicator.dataset.tid = "call-transcript-indicator";
            document.body.append(indicator);
          }),
        ]),
      ),
      item("LanguageSpeechMenuControl-id", "menuitem", "Language and speech", () =>
        showSubmenu([
          item(
            "closed-captions-button",
            "menuitemcheckbox",
            "<span>Show live captions</span><span>Alt+Shift+C</span>",
            () => {
              const captions = document.createElement("div");
              captions.dataset.tid = "closed-caption-renderer-wrapper";
              document.body.append(captions);
            },
          ),
        ]),
      ),
    );
    document.body.append(menu);
  });
}

/** Whether the More menu is open. */
function moreMenuOpen(): boolean {
  return document.getElementById("callingButtons-showMoreBtn")?.getAttribute("aria-expanded") === "true";
}

describe("findControl", () => {
  it("ignores a keyboard shortcut shown after the label", () => {
    document.body.innerHTML =
      '<div role="menuitemcheckbox"><span>Show live captions</span><span>Alt+Shift+C</span></div>';
    expect(findControl(document, /^show live captions$/i)).not.toBeNull();
  });
});

describe("Teams call automation", () => {
  beforeEach(buildTeamsCall);

  it("turns on live captions through Language and speech and closes the menu", async () => {
    expect(await turnOnCaptions(document)).toBe(true);
    expect(document.querySelector('[data-tid="closed-caption-renderer-wrapper"]')).not.toBeNull();
    expect(moreMenuOpen()).toBe(false);
  });

  it("starts transcription after captions, one step after the other", async () => {
    const captions = turnOnCaptions(document);
    const transcription = startTeamsTranscription(document);
    expect(await captions).toBe(true);
    expect(await transcription).toBe(true);
    expect(moreMenuOpen()).toBe(false);
  });

  it("uses a More menu someone left open instead of closing it", async () => {
    document.getElementById("callingButtons-showMoreBtn")?.click();
    expect(moreMenuOpen()).toBe(true);
    expect(await startTeamsTranscription(document)).toBe(true);
    expect(document.querySelector('[data-tid="call-transcript-indicator"]')).not.toBeNull();
  });
});
