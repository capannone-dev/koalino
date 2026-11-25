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
    const fetched = await fetchRecapTranscript(document, null, null, () => (requested += 1), async () => false);
    expect(fetched).toBe("nothing-arrived");
    expect(requested).toBe(3);
  });

  it("reports a recap without a Transcript tab", async () => {
    document.body.innerHTML = "<div>AI summary</div>";
    const fetched = await fetchRecapTranscript(document, null, null, () => undefined, async () => true, 300);
    expect(fetched).toBe("no-transcript-tab");
  });

  /**
   * A recap view on `shown`, with a "Select the meeting by time" picker listing `sessions` as
   * options (labelled as Teams does, live markup of 2026-11-24). Choosing one shows it.
   */
  function buildSessionPicker(shown: string, sessions: string[]): void {
    document.body.innerHTML = `
      <button role="combobox" aria-label="Select the meeting by time" aria-expanded="false">${shown}</button>
      <button role="tab" data-tid="Transcript">Transcript</button>`;
    const picker = document.querySelector<HTMLElement>('[role="combobox"]') as HTMLElement;
    picker.addEventListener("click", () => {
      document.getElementById("sessions")?.remove();
      const list = document.createElement("div");
      list.id = "sessions";
      list.setAttribute("role", "listbox");
      for (const session of sessions) {
        list.append(
          item(`option-${list.childElementCount}`, "option", session, () => {
            picker.textContent = session;
            list.remove();
          }),
        );
      }
      document.body.append(list);
    });
  }

  const at = (hours: number, minutes: number) => new Date(2026, 10, 24, hours, minutes).getTime();
  const EARLIER = "Tuesday, November 24, 2026 2:24 PM - 3:05 PM";
  const RECORDED = "Tuesday, November 24, 2026 5:16 PM - 5:18 PM";

  it("switches the recap to the recorded session before downloading (#158)", async () => {
    buildSessionPicker(EARLIER, [EARLIER, RECORDED]);
    let requested = 0;
    const fetched = await fetchRecapTranscript(
      document,
      null,
      { startMs: at(17, 11), endMs: at(17, 18) },
      () => {
        // The download must come from the recorded session.
        expect(document.querySelector('[role="combobox"]')?.textContent).toBe(RECORDED);
        requested += 1;
      },
      async () => true,
    );
    expect(fetched).toBe("sent");
    expect(requested).toBe(1);
  });

  it("downloads nothing when no session matches the recording (#158)", async () => {
    buildSessionPicker(EARLIER, [EARLIER]);
    let requested = 0;
    const fetched = await fetchRecapTranscript(
      document,
      null,
      { startMs: at(17, 11), endMs: at(17, 18) },
      () => (requested += 1),
      async () => true,
      300,
    );
    expect(fetched).toBe("wrong-session");
    expect(requested).toBe(0);
  });

  it("waits for a session picker that appears after the Transcript tab (#159)", async () => {
    // Right after a call Teams shows the recap's Transcript tab before its session picker.
    document.body.innerHTML = '<button role="tab" data-tid="Transcript">Transcript</button>';
    setTimeout(() => {
      const tab = document.querySelector("button");
      buildSessionPicker(EARLIER, [EARLIER]);
      if (tab) document.querySelector('[role="tab"]')?.replaceWith(tab);
    }, 100);
    let requested = 0;
    const fetched = await fetchRecapTranscript(
      document,
      null,
      { startMs: at(17, 11), endMs: at(17, 18) },
      () => (requested += 1),
      async () => true,
      1_000,
    );
    expect(fetched).toBe("wrong-session");
    expect(requested).toBe(0);
  });

  it("downloads from a recap with no session picker once none appears", async () => {
    document.body.innerHTML = '<button role="tab" data-tid="Transcript">Transcript</button>';
    let requested = 0;
    const fetched = await fetchRecapTranscript(
      document,
      null,
      { startMs: at(17, 11), endMs: at(17, 18) },
      () => (requested += 1),
      async () => true,
      300,
    );
    expect(fetched).toBe("sent");
    expect(requested).toBe(1);
  });

  it("keeps a recap already showing the recorded session", async () => {
    buildSessionPicker(RECORDED, []);
    const fetched = await fetchRecapTranscript(
      document,
      null,
      { startMs: at(17, 11), endMs: at(17, 18) },
      () => undefined,
      async () => true,
    );
    expect(fetched).toBe("sent");
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

/**
 * A model of the captions pane's settings menu, as Teams showed it on 2026-11-24: the gear
 * (`closed-captions-settings-menu-trigger-button`) opens a menu whose "Meeting spoken language"
 * item opens a list of radio items, one per locale (`…-spoken-language-it-it`). Choosing one
 * applies it at once. `applied` holds the chosen locale.
 */
function buildCaptionsLanguageMenu(current: string, locales: Record<string, string>): { applied: string | null } {
  const result: { applied: string | null } = { applied: null };
  document.body.innerHTML =
    '<div data-tid="closed-caption-v2-window-wrapper"><button data-tid="closed-captions-settings-menu-trigger-button" aria-haspopup="menu" aria-expanded="false" aria-label="Live Caption, Caption Settings"></button></div>';
  const gear = document.querySelector<HTMLElement>('[data-tid="closed-captions-settings-menu-trigger-button"]')!;
  const closeAll = () => {
    document.getElementById("captions-menu")?.remove();
    document.getElementById("submenu")?.remove();
    gear.setAttribute("aria-expanded", "false");
  };
  gear.addEventListener("click", () => {
    if (gear.getAttribute("aria-expanded") === "true") {
      closeAll();
      return;
    }
    gear.setAttribute("aria-expanded", "true");
    const menu = document.createElement("div");
    menu.id = "captions-menu";
    menu.setAttribute("role", "menu");
    const spoken = document.createElement("div");
    spoken.setAttribute("role", "menuitem");
    spoken.setAttribute("data-tid", "captions-settings-redesign-spoken-language");
    spoken.setAttribute("aria-haspopup", "menu");
    spoken.innerHTML = `<span>Meeting spoken language</span><span>${current}</span>`;
    spoken.addEventListener("click", () => {
      const items = Object.entries(locales).map(([locale, name]) => {
        const radio = document.createElement("div");
        radio.setAttribute("role", "menuitemradio");
        radio.setAttribute("data-tid", `captions-settings-redesign-spoken-language-${locale}`);
        radio.setAttribute("aria-checked", String(name === current));
        radio.textContent = name;
        radio.addEventListener("click", () => {
          result.applied = locale;
          closeAll();
        });
        return radio;
      });
      showSubmenu(items);
    });
    menu.append(spoken);
    document.body.append(menu);
  });
  return result;
}

describe("Teams spoken language from the captions settings (#141)", () => {
  const locales = {
    "en-us": "English (US)",
    "fr-fr": "French (France)",
    "it-it": "Italian (Italy)",
    "it-ch": "Italian (Switzerland)",
  };

  it("chooses Koalino's language in the captions settings", async () => {
    const teams = buildCaptionsLanguageMenu("English (US)", locales);
    expect(await setTeamsSpokenLanguage(document, "it")).toBe("set");
    expect(teams.applied).toBe("it-it");
    expect(document.querySelector("[role='menu']")).toBeNull();
  });

  it("leaves a language that already matches alone and closes the menu", async () => {
    const teams = buildCaptionsLanguageMenu("Italian (Italy)", locales);
    expect(await setTeamsSpokenLanguage(document, "it")).toBe("already");
    expect(teams.applied).toBeNull();
    expect(document.querySelector("[role='menu']")).toBeNull();
  });

  it("closes the menu when Teams does not offer the language", async () => {
    const teams = buildCaptionsLanguageMenu("English (US)", locales);
    expect(await setTeamsSpokenLanguage(document, "fi")).toBe("failed");
    expect(teams.applied).toBeNull();
    expect(document.querySelector("[role='menu']")).toBeNull();
  });
});
