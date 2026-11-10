/**
 * The extension's background (Issue #102): the only part that talks to the native messaging
 * host. Teams tabs connect over a port and send `host` messages, which go to the host in order;
 * the host's status replies are passed to every tab and to the popup.
 *
 * The host is started on the first message (or when the popup checks the connection) and kept
 * while the background runs; after the browser stops it, the next message starts it again. In
 * Chrome the background is a service worker that open ports keep alive; in Firefox it is an event
 * page. Message contents are never logged.
 *
 * It also relays between a tab and its recap frames (the SharePoint frame Teams shows a recap
 * transcript in): the tab asks them to download the transcript, and they pass back the WebVTT
 * their page hook copied.
 *
 * After it is installed without access to the Teams sites (Firefox, when loaded by hand), it opens
 * its settings page in a tab, where the owner can allow it.
 */
import {
  BackgroundMessage,
  ConnectionState,
  HOST_NAME,
  HostMessage,
  HostReply,
  PORT_NAME,
  RECAP_FRAME_PORT_NAME,
  RecapFrameCommand,
  RecapFrameMessage,
  TabMessage,
  describeProgress,
} from "../lib/messages";
import { hasTeamsAccess } from "../lib/siteAccess";

/** Requests the popup sends. */
type PopupRequest = { kind: "get-state" } | { kind: "check-host" };

/** What the popup shows. */
export interface PopupState {
  connection: ConnectionState;
  recordings: string[];
  /** Each tab's last Teams transcript fetch status. */
  recapStatuses: string[];
  rejected: number;
}

let hostPort: chrome.runtime.Port | null = null;
let connection: ConnectionState = { host: "unknown", bridge: "unknown" };
let rejected = 0;
const tabs = new Set<chrome.runtime.Port>();
/** What each recording tab shows in the popup: its title, then its progress line. */
const recordings = new Map<chrome.runtime.Port, string>();
/** Each tab's last Teams transcript fetch status, for the popup. */
const recapStatuses = new Map<chrome.runtime.Port, string>();

/** Tells every tab the connection state. */
function broadcast(): void {
  const message: BackgroundMessage = { kind: "connection", state: connection };
  for (const tab of tabs) {
    try {
      tab.postMessage(message);
    } catch {
      tabs.delete(tab);
    }
  }
}

/** Updates the connection state and tells the tabs when it changed. */
function setConnection(next: ConnectionState): void {
  if (JSON.stringify(next) === JSON.stringify(connection)) return;
  connection = next;
  broadcast();
}

/** Starts the native host when it is not running. Outputs: The port, or null. */
function ensureHost(): chrome.runtime.Port | null {
  if (hostPort) return hostPort;
  try {
    const port = chrome.runtime.connectNative(HOST_NAME);
    port.onMessage.addListener((reply: HostReply) => {
      if (reply.type === "status") setConnection({ host: "connected", bridge: reply.bridge });
      if (reply.type === "rejected") rejected += 1;
    });
    port.onDisconnect.addListener(() => {
      const reason = chrome.runtime.lastError?.message;
      hostPort = null;
      setConnection({ host: "missing", bridge: "unknown", ...(reason ? { hostError: reason } : {}) });
    });
    hostPort = port;
    setConnection({ host: "connected", bridge: connection.bridge });
    return port;
  } catch (error) {
    setConnection({ host: "missing", bridge: "unknown", hostError: String(error) });
    return null;
  }
}

/** Sends one message to the host, starting it if needed. */
function sendToHost(message: HostMessage): void {
  const port = ensureHost();
  try {
    port?.postMessage(message);
  } catch {
    hostPort = null;
    ensureHost()?.postMessage(message);
  }
}

/** The recap frames of each tab (by tab ID): SharePoint frames a Teams page opened. */
const recapFrames = new Map<number, Set<chrome.runtime.Port>>();

/** Relays a recap frame script's messages to its tab's content script. */
function connectRecapFrame(port: chrome.runtime.Port): void {
  const tabId = port.sender?.tab?.id;
  if (tabId === undefined) return;
  const frames = recapFrames.get(tabId) ?? new Set<chrome.runtime.Port>();
  frames.add(port);
  recapFrames.set(tabId, frames);
  port.onMessage.addListener((message: RecapFrameMessage) => {
    if (message.kind !== "recap-transcript" || typeof message.text !== "string") return;
    for (const tab of tabs) {
      if (tab.sender?.tab?.id === tabId && (tab.sender.frameId ?? 0) === 0) {
        tab.postMessage({ kind: "recap-transcript", text: message.text } satisfies BackgroundMessage);
      }
    }
  });
  port.onDisconnect.addListener(() => {
    frames.delete(port);
    if (frames.size === 0) recapFrames.delete(tabId);
  });
}

/** Asks the recap frames of a tab to download the recap transcript. */
function requestRecapDownload(tabId: number | undefined): void {
  if (tabId === undefined) return;
  for (const frame of recapFrames.get(tabId) ?? []) {
    try {
      frame.postMessage({ kind: "recap-download" } satisfies RecapFrameCommand);
    } catch {
      // The frame went away; its disconnect removes it.
    }
  }
}

chrome.runtime.onConnect.addListener((port) => {
  if (port.name === RECAP_FRAME_PORT_NAME) {
    connectRecapFrame(port);
    return;
  }
  if (port.name !== PORT_NAME) return;
  tabs.add(port);
  port.postMessage({ kind: "connection", state: connection } satisfies BackgroundMessage);
  port.onMessage.addListener((message: TabMessage) => {
    if (message.kind === "host") sendToHost(message.message);
    if (message.kind === "recording") {
      if (message.recording) recordings.set(port, message.title || "Teams meeting");
      else recordings.delete(port);
    }
    if (message.kind === "progress" && recordings.has(port)) recordings.set(port, describeProgress(message.progress));
    if (message.kind === "recap-download") requestRecapDownload(port.sender?.tab?.id);
    if (message.kind === "recap-status") recapStatuses.set(port, message.status);
  });
  port.onDisconnect.addListener(() => {
    tabs.delete(port);
    recordings.delete(port);
    recapStatuses.delete(port);
  });
});

chrome.runtime.onMessage.addListener((request: PopupRequest, _sender, respond) => {
  if (request.kind === "check-host") ensureHost();
  const state: PopupState = {
    connection,
    recordings: [...recordings.values()],
    recapStatuses: [...recapStatuses.values()],
    rejected,
  };
  respond(state);
  return false;
});

chrome.runtime.onInstalled.addListener(() => {
  void hasTeamsAccess(chrome.permissions, chrome.runtime.getManifest()).then((granted) => {
    if (!granted) void chrome.tabs.create({ url: chrome.runtime.getURL("popup.html") });
  });
});
