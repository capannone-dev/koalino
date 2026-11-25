/**
 * The popup (Issue #102): shows whether the Koalino helper (native host) and app are
 * reachable, which meetings are being recorded, what happened to the last Teams transcript fetch,
 * and the settings as checkboxes. When the extension may not run on the meeting sites, it says so
 * first and offers to allow it.
 *
 * Since #174 it also records the current tab on any site ("Record this tab", through the
 * background), lets the owner watch every website for calls (asking the browser for access to
 * all sites), and shows and forgets the per-site Always / Never choices.
 */
import type { PopupRequest, PopupState, TabRecordingReply } from "../background/background";
import { ANY_SITE_PATTERNS } from "../lib/anySite";
import { msg, textDirection, uiLanguage, type MessageKey } from "../lib/i18n";
import { ExtensionSettings } from "../lib/settings";
import { hasTeamsAccess, requestTeamsAccess } from "../lib/siteAccess";
import { MEETING_SERVICES } from "../lib/sites";
import { loadSettings, saveSettings } from "../lib/storage";

/** The tab the popup was opened on, or null (a window without tabs). */
let currentTabId: number | null = null;
/** Whether that tab records now. */
let currentTabRecording = false;

/** Renders the connection and recordings. */
function render(state: PopupState): void {
  const host = document.getElementById("host");
  const recordings = document.getElementById("recordings");
  if (!host || !recordings) return;
  const { connection } = state;
  if (connection.host === "missing") {
    host.className = "bad";
    host.textContent = msg("helperMissing");
  } else if (connection.bridge === "connected") {
    host.className = "ok";
    host.textContent = msg("connectedToApp");
  } else if (connection.bridge === "unavailable") {
    host.className = "warn";
    host.textContent = msg("appNotRunning");
  } else {
    host.className = "";
    host.textContent = msg("notCheckedYet");
  }
  recordings.textContent =
    state.recordings.length > 0
      ? msg("recordingMeetings", { meetings: state.recordings.join("\n") })
      : msg("notRecording");
  const transcripts = document.getElementById("transcripts");
  if (transcripts) {
    transcripts.textContent = state.recapStatuses.join("\n");
    transcripts.hidden = state.recapStatuses.length === 0;
  }
  renderTabButton(currentTabId !== null && state.recordingTabIds.includes(currentTabId));
}

/** Shows "Record this tab" or "Stop recording this tab". */
function renderTabButton(recording: boolean): void {
  currentTabRecording = recording;
  const button = document.getElementById("record-tab");
  if (!button) return;
  button.textContent = msg(recording ? "stopThisTab" : "recordThisTab");
  button.toggleAttribute("disabled", currentTabId === null);
}

/**
 * Records the current tab, or stops it, through the background.
 * Side effects: The background injects the scripts and tells the tab; updates the button.
 */
async function toggleTabRecording(): Promise<void> {
  if (currentTabId === null) return;
  const request: PopupRequest = { kind: currentTabRecording ? "stop-tab" : "record-tab", tabId: currentTabId };
  const reply = (await chrome.runtime.sendMessage(request)) as TabRecordingReply;
  document.getElementById("record-tab-problem")?.toggleAttribute("hidden", reply.failed !== true);
  renderTabButton(reply.recording);
}

/** A site key as the owner knows it: the service's name, else the host. */
function siteLabel(key: string): string {
  return MEETING_SERVICES.find((service) => service.id === key)?.name ?? key;
}

/** Shows the per-site choices, or hides the line when there are none. */
function renderSiteChoices(settings: ExtensionSettings): void {
  const line = document.getElementById("site-choices");
  const text = document.getElementById("site-choices-text");
  if (!line || !text) return;
  const list = (keys: string[]) => (keys.length > 0 ? keys.map(siteLabel).join(", ") : msg("noSites"));
  text.textContent = msg("siteChoices", { always: list(settings.alwaysRecordSites), never: list(settings.neverAskSites) });
  line.hidden = settings.alwaysRecordSites.length === 0 && settings.neverAskSites.length === 0;
}

/** Asks the background for its state. */
async function refresh(kind: "get-state" | "check-host"): Promise<void> {
  const state = (await chrome.runtime.sendMessage({ kind })) as PopupState;
  render(state);
}

/** The settings shown as checkboxes. */
type SwitchSetting = { [Key in keyof ExtensionSettings]: ExtensionSettings[Key] extends boolean ? Key : never }[keyof ExtensionSettings];

/** Binds the checkboxes to the settings. */
async function bindSettings(): Promise<void> {
  const settings = await loadSettings();
  for (const input of document.querySelectorAll<HTMLInputElement>("input[data-setting]")) {
    const key = input.dataset.setting as SwitchSetting;
    input.checked = settings[key];
    input.addEventListener("change", () => {
      void saveSettings({ [key]: input.checked });
    });
  }
  renderSiteChoices(settings);
  document.getElementById("forget-sites")?.addEventListener("click", () => {
    void saveSettings({ alwaysRecordSites: [], neverAskSites: [] }).then(renderSiteChoices);
  });
  await bindWatchAllSites(settings);
}

/**
 * Binds "Watch every website for calls": on, it asks the browser for access to all sites first.
 * Side effects: May show the browser's permission prompt; writes the setting.
 */
async function bindWatchAllSites(settings: ExtensionSettings): Promise<void> {
  const input = document.getElementById("watch-all-sites") as HTMLInputElement | null;
  if (!input) return;
  input.checked = settings.watchAllSites && (await chrome.permissions.contains({ origins: ANY_SITE_PATTERNS }));
  input.addEventListener("change", () => {
    if (!input.checked) {
      void saveSettings({ watchAllSites: false });
      return;
    }
    // The request must start in the click itself, before any other await.
    void chrome.permissions.request({ origins: ANY_SITE_PATTERNS }).then((granted) => {
      input.checked = granted;
      void saveSettings({ watchAllSites: granted });
    });
  });
}

/**
 * Shows the Teams access notice when access is missing.
 *
 * Inputs: Whether the request just made was refused. Side effects: Changes the page.
 */
async function renderAccess(refused = false): Promise<void> {
  const notice = document.getElementById("access");
  const text = document.getElementById("access-text");
  if (!notice || !text) return;
  const granted = await hasTeamsAccess(chrome.permissions, chrome.runtime.getManifest());
  notice.hidden = granted && !justGranted;
  if (justGranted) {
    notice.className = "status ok";
    text.textContent = msg("accessAllowed");
    document.getElementById("allow")?.setAttribute("hidden", "");
  } else if (refused) {
    text.textContent = msg("accessRefused");
  }
}

/** Set once access was granted from this page, to tell the owner to reload Teams. */
let justGranted = false;

/**
 * Puts the popup's fixed text in the browser's language.
 *
 * Inputs: None. Side effects: Sets `lang` and `dir` on the page and the text of every element
 * marked `data-i18n`.
 */
function translatePage(): void {
  document.documentElement.lang = uiLanguage();
  document.documentElement.dir = textDirection();
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n]")) {
    element.textContent = msg(element.dataset.i18n as MessageKey);
  }
}

translatePage();
document.getElementById("allow")?.addEventListener("click", () => {
  // The request must start in the click itself, before any other await.
  void requestTeamsAccess(chrome.permissions, chrome.runtime.getManifest()).then((granted) => {
    justGranted = granted;
    void renderAccess(!granted);
  });
});

document.getElementById("record-tab")?.addEventListener("click", () => void toggleTabRecording());
document.getElementById("check")?.addEventListener("click", () => {
  void refresh("check-host");
  setTimeout(() => void refresh("get-state"), 1_500);
});
void renderAccess();
void bindSettings();
void chrome.tabs
  .query({ active: true, currentWindow: true })
  .then(([tab]) => {
    // Opened in a tab of its own (after install), the popup has no call tab to record.
    const ownPage = tab?.url?.startsWith(chrome.runtime.getURL("")) ?? false;
    currentTabId = tab?.id !== undefined && !ownPage ? tab.id : null;
  })
  .catch(() => undefined)
  .finally(() => void refresh("get-state"));
