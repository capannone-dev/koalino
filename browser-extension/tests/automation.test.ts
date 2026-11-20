/**
 * Specifies the Teams automation against a model of the call's "More" menu as the #102 live test
 * in Firefox showed it (work account, November 2026): the button toggles a menu, submenus open on
 * click, and items carry stable IDs and, for some, a keyboard shortcut after their label.
 */
import { beforeEach, describe, expect, it } from "vitest";
import {
  downloadRecapVtt,
  fetchRecapTranscript,
  findControl,
  setTeamsSpokenLanguage,
  startTeamsTranscription,
  turnOnCaptions,
} from "../src/content/automation";

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
  // An unselected Fluent tab holds its label twice: the visible text and a hidden bold copy that
  // reserves its width, so its text reads "TranscriptTranscript".
  it("reads a Fluent tab's doubled label once", () => {
    document.body.innerHTML =
      '<button role="tab" aria-selected="false"><span>Transcript</span><span aria-hidden="true">Transcript</span></button>';
    expect(findControl(document, /^transcript$/i)).not.toBeNull();
  });

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

describe("recap transcript", () => {
  /** The recap frame's Download button, whose menu offers the formats. */
  function buildRecapFrame(): string[] {
    const downloads: string[] = [];
    document.body.innerHTML = '<button aria-haspopup="menu">Download</button>';
    document.querySelector("button")?.addEventListener("click", () =>
      showSubmenu([
        item("docx", "menuitem", "Download as .docx", () => downloads.push("docx")),
        item("vtt", "menuitem", "Download as .vtt", () => downloads.push("vtt")),
      ]),
    );
    return downloads;
  }

  it("downloads the WebVTT file from the recap frame", async () => {
    const downloads = buildRecapFrame();
    expect(await downloadRecapVtt(document)).toBe(true);
    expect(downloads).toEqual(["vtt"]);
  });

  it("reports a recap frame without a Download button", async () => {
    document.body.innerHTML = "<div>Loading</div>";
    expect(await downloadRecapVtt(document, 300)).toBe(false);
  });

  it("opens the Transcript tab and asks the recap frame to download", async () => {
    document.body.innerHTML =
      '<button role="tab" aria-selected="false" data-tid="Transcript"><span>Transcript</span><span>Transcript</span></button>';
    let tabOpened = false;
    document.querySelector("button")?.addEventListener("click", () => {
      tabOpened = true;
    });
    let requested = 0;
    const arrivals = [false, true];
    const fetched = await fetchRecapTranscript(
      document,
      null,
      () => {
        requested += 1;
      },
      async () => arrivals.shift() ?? false,
    );
    expect(fetched).toBe("sent");
    expect(tabOpened).toBe(true);
    expect(requested).toBe(2);
  });

  it("gives up when no transcript arrives", async () => {
    document.body.innerHTML = '<button role="tab" data-tid="Transcript">Transcript</button>';
    let requested = 0;
    const fetched = await fetchRecapTranscript(document, null, () => (requested += 1), async () => false);
    expect(fetched).toBe("nothing-arrived");
    expect(requested).toBe(3);
  });

  it("reports a recap without a Transcript tab", async () => {
    document.body.innerHTML = "<div>AI summary</div>";
    const fetched = await fetchRecapTranscript(document, null, () => undefined, async () => true, 300);
    expect(fetched).toBe("no-transcript-tab");
  });
});

/**
 * A model of Teams' spoken-language dialog: More → Language and speech → Change spoken language
 * opens a dialog with a "Meeting spoken language" dropdown and an Update button. `applied` holds
 * what Update saved.
 */
function buildLanguageDialog(current: string, options: string[]): { applied: string | null } {
  const result: { applied: string | null } = { applied: null };
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
      item("LanguageSpeechMenuControl-id", "menuitem", "Language and speech", () =>
        showSubmenu([
          item("spoken-language-button", "menuitem", "Change spoken language", () => {
            const dialog = document.createElement("div");
            dialog.setAttribute("role", "dialog");
            const dropdown = document.createElement("button");
            dropdown.setAttribute("role", "combobox");
            dropdown.setAttribute("aria-label", "Meeting spoken language");
            dropdown.textContent = current;
            dropdown.addEventListener("click", () => {
              const list = document.createElement("div");
              list.setAttribute("role", "listbox");
              for (const name of options) {
                const option = document.createElement("div");
                option.setAttribute("role", "option");
                option.textContent = name;
                option.addEventListener("click", () => {
                  dropdown.textContent = name;
                  list.remove();
                });
                list.append(option);
              }
              document.body.append(list);
            });
            const update = document.createElement("button");
            update.textContent = "Update";
            update.addEventListener("click", () => {
              result.applied = dropdown.textContent;
              dialog.remove();
            });
            const cancel = document.createElement("button");
            cancel.textContent = "Cancel";
            cancel.addEventListener("click", () => dialog.remove());
            dialog.append(dropdown, cancel, update);
            document.body.append(dialog);
          }),
        ]),
      ),
    );
    document.body.append(menu);
  });
  return result;
}

describe("Teams spoken language (#141)", () => {
  const languages = ["English (United States)", "French (France)", "Italian (Italy)", "Italian (Switzerland)"];

  it("chooses Koalino's language and presses Update", async () => {
    const teams = buildLanguageDialog("English (United States)", languages);
    expect(await setTeamsSpokenLanguage(document, "it")).toBe("set");
    expect(teams.applied).toBe("Italian (Italy)");
    expect(document.querySelector("[role='dialog']")).toBeNull();
  });

  it("leaves a language that already matches alone", async () => {
    const teams = buildLanguageDialog("Italian (Italy)", languages);
    expect(await setTeamsSpokenLanguage(document, "it")).toBe("already");
    expect(teams.applied).toBeNull();
    expect(document.querySelector("[role='dialog']")).toBeNull();
  });

  it("cancels when Teams does not offer the language", async () => {
    const teams = buildLanguageDialog("English (United States)", languages);
    expect(await setTeamsSpokenLanguage(document, "fi")).toBe("failed");
    expect(teams.applied).toBeNull();
    expect(document.querySelector("[role='dialog']")).toBeNull();
  });
});
