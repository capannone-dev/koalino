/**
 * The recap frame script (Issue #102): runs in SharePoint frames, and acts only in one a Teams
 * page opened. Teams shows a meeting recap's transcript in such a frame (Stream's player). The
 * player's own copy of the transcript is encrypted, so the only WebVTT is what its Download →
 * "Download as .vtt" fetches; the page hook in this frame copies that response.
 *
 * - On the background's `recap-download` it presses those two controls (`downloadRecapVtt`). The
 *   browser also keeps the file in its downloads.
 * - It passes each WebVTT file the hook copied to the background, which hands it to the Teams
 *   tab's content script; that one decides which recorded meeting it belongs to.
 *
 * Nothing of the transcript is logged or kept here.
 */
import { downloadRecapVtt } from "../content/automation";
import {
  HOOK_SOURCE,
  HookMessage,
  RECAP_FRAME_PORT_NAME,
  RecapFrameCommand,
  RecapFrameMessage,
} from "../lib/messages";
import { embeddedInTeams } from "../lib/siteAccess";

/** Whether a download is running, so repeated requests do not save the file twice at once. */
let downloading = false;
/** The port to the background; opened again when Firefox stops an idle background. */
let port: chrome.runtime.Port | null = null;

/**
 * Opens the port to the background, so it can ask this frame to download.
 * Side effects: Opens a port; presses the frame's Download controls when asked.
 */
function connect(): chrome.runtime.Port {
  const opened = chrome.runtime.connect({ name: RECAP_FRAME_PORT_NAME });
  opened.onMessage.addListener((command: RecapFrameCommand) => {
    if (command.kind !== "recap-download" || downloading) return;
    downloading = true;
    void downloadRecapVtt(document).finally(() => {
      downloading = false;
    });
  });
  opened.onDisconnect.addListener(() => {
    port = null;
    // A stopped background: reconnect so the next request reaches this frame.
    port = connect();
  });
  return opened;
}

/**
 * Connects to the background and relays in both directions.
 * Side effects: Opens a port; listens to the page hook.
 */
function start(): void {
  port = connect();
  window.addEventListener("message", (event: MessageEvent<HookMessage>) => {
    if (event.source !== window || event.data?.source !== HOOK_SOURCE) return;
    if (event.data.kind !== "transcript-file") return;
    if (!event.data.text.trimStart().startsWith("WEBVTT")) return;
    const message = { kind: "recap-transcript", text: event.data.text } satisfies RecapFrameMessage;
    try {
      (port ?? connect()).postMessage(message);
    } catch {
      try {
        port = connect();
        port.postMessage(message);
      } catch {
        // The extension was reloaded or removed; nothing to relay to.
      }
    }
  });
}

if (embeddedInTeams(window.top === window, document.referrer)) start();
