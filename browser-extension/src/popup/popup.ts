/**
 * The popup (Issue #102): shows whether the Koalino helper (native host) and app are
 * reachable, which meetings are being recorded, and what happened to the last Teams transcript
 * fetch. When the extension may not run on the meeting sites, it says so first and offers to
 * allow it. Since #174 it records the current tab on any site ("Record this tab").
 *
 * Since #212 it checks the connection by itself when opened, has one main button (Record this tab
 * or Stop), and leaves the settings to the options page, opened by its Settings link.
 */
import type { PopupRequest, PopupState, TabRecordingReply } from "../background/background";
import { msg } from "../lib/i18n";
import { translatePage } from "../lib/pageText";
import { addIcons, setIconLabel } from "../lib/icons";
import { connectionStatus } from "../lib/popupStatus";
import { hasTeamsAccess, requestTeamsAccess } from "../lib/siteAccess";

/** The tab the popup was opened on, or null (a window without tabs). */
let currentTabId: number | null = null;
/** Whether that tab records now. */
let currentTabRecording = false;

/** Renders the connection and recordings. */
function render(state: PopupState): void {
  const host = document.getElementById("host");
  const recordings = document.getElementById("recordings");
  if (!host || !recordings) return;
  const status = connectionStatus(state.connection);
  host.className = status.tone;
  // An icon beside the words, so the state does not rest on colour alone (#207).
  if (status.tone === "ok") setIconLabel(host, "check", msg(status.key));
  else if (status.tone === "checking") host.textContent = msg(status.key);
  else setIconLabel(host, "error", msg(status.key));
  // "Check again" only when something is wrong; a working connection needs no button.
  document.getElementById("check")?.toggleAttribute("hidden", status.tone === "ok" || status.tone === "checking");
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
  setIconLabel(button, recording ? "stop" : "mic", msg(recording ? "stopThisTab" : "recordThisTab"));
  button.classList.toggle("stop", recording);
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

/** Asks the background for its state. */
async function refresh(kind: "get-state" | "check-host"): Promise<void> {
  const state = (await chrome.runtime.sendMessage({ kind })) as PopupState;
  render(state);
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

translatePage();
addIcons();
document.getElementById("allow")?.addEventListener("click", () => {
  // The request must start in the click itself, before any other await.
  void requestTeamsAccess(chrome.permissions, chrome.runtime.getManifest()).then((granted) => {
    justGranted = granted;
    void renderAccess(!granted);
  });
});

document.getElementById("record-tab")?.addEventListener("click", () => void toggleTabRecording());
/** Asks the background to reach the helper and app, then shows what it found. */
function checkConnection(): void {
  void refresh("check-host");
  setTimeout(() => void refresh("get-state"), 1_500);
}

document.getElementById("check")?.addEventListener("click", checkConnection);
document.getElementById("open-settings")?.addEventListener("click", (event) => {
  event.preventDefault();
  void chrome.runtime.openOptionsPage();
});
void renderAccess();
void chrome.tabs
  .query({ active: true, currentWindow: true })
  .then(([tab]) => {
    // Opened in a tab of its own (after install), the popup has no call tab to record.
    const ownPage = tab?.url?.startsWith(chrome.runtime.getURL("")) ?? false;
    currentTabId = tab?.id !== undefined && !ownPage ? tab.id : null;
  })
  .catch(() => undefined)
  // Checked on every opening, so the owner never has to press a button to see the status.
  .finally(checkConnection);
