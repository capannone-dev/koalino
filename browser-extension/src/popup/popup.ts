/**
 * The popup (Issue #102): shows whether the Note Taker helper (native host) and app are
 * reachable, which meetings are being recorded, and the settings as checkboxes.
 */
import type { PopupState } from "../background/background";
import { ExtensionSettings } from "../lib/settings";
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
    state.recordings.length > 0 ? `Recording: ${state.recordings.join(", ")}` : "Not recording.";
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

document.getElementById("check")?.addEventListener("click", () => {
  void refresh("check-host");
  setTimeout(() => void refresh("get-state"), 1_500);
});
void bindSettings();
void refresh("get-state");
