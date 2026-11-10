/**
 * The popup (Issue #102): shows whether the Note Taker helper (native host) and app are
 * reachable, which meetings are being recorded, what happened to the last Teams transcript fetch,
 * and the settings as checkboxes. When the extension
 * may not run on the Teams sites, it says so first and offers to allow it.
 */
import type { PopupState } from "../background/background";
import { ExtensionSettings } from "../lib/settings";
import { hasTeamsAccess, requestTeamsAccess } from "../lib/siteAccess";
import { loadSettings, saveSettings } from "../lib/storage";

/** Renders the connection and recordings. */
function render(state: PopupState): void {
  const host = document.getElementById("host");
  const recordings = document.getElementById("recordings");
  if (!host || !recordings) return;
  const { connection } = state;
  if (connection.host === "missing") {
    host.className = "bad";
    host.textContent = "Helper not installed: run “browser-host install” from Note Taker.";
  } else if (connection.bridge === "connected") {
    host.className = "ok";
    host.textContent = "Connected to the Note Taker app.";
  } else if (connection.bridge === "unavailable") {
    host.className = "warn";
    host.textContent = "Note Taker app not running (or its meeting bridge is off); recordings wait.";
  } else {
    host.className = "";
    host.textContent = "Not checked yet.";
  }
  recordings.textContent =
    state.recordings.length > 0 ? `Recording ${state.recordings.join("\n")}` : "Not recording.";
  const transcripts = document.getElementById("transcripts");
  if (transcripts) {
    transcripts.textContent = state.recapStatuses.join("\n");
    transcripts.hidden = state.recapStatuses.length === 0;
  }
}

/** Asks the background for its state. */
async function refresh(kind: "get-state" | "check-host"): Promise<void> {
  const state = (await chrome.runtime.sendMessage({ kind })) as PopupState;
  render(state);
}

/** Binds the checkboxes to the settings. */
async function bindSettings(): Promise<void> {
  const settings = await loadSettings();
  for (const input of document.querySelectorAll<HTMLInputElement>("input[data-setting]")) {
    const key = input.dataset.setting as keyof ExtensionSettings;
    input.checked = settings[key];
    input.addEventListener("change", () => {
      void saveSettings({ [key]: input.checked });
    });
  }
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
    text.textContent = "Allowed. Reload any Teams tab that was already open.";
    document.getElementById("allow")?.setAttribute("hidden", "");
  } else if (refused) {
    text.textContent = "Not allowed: Note Taker cannot record Teams until you allow it.";
  }
}

/** Set once access was granted from this page, to tell the owner to reload Teams. */
let justGranted = false;

document.getElementById("allow")?.addEventListener("click", () => {
  // The request must start in the click itself, before any other await.
  void requestTeamsAccess(chrome.permissions, chrome.runtime.getManifest()).then((granted) => {
    justGranted = granted;
    void renderAccess(!granted);
  });
});

document.getElementById("check")?.addEventListener("click", () => {
  void refresh("check-host");
  setTimeout(() => void refresh("get-state"), 1_500);
});
void renderAccess();
void bindSettings();
void refresh("get-state");
