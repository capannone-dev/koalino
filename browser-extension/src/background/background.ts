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
  rejected: number;
}

let hostPort: chrome.runtime.Port | null = null;
let connection: ConnectionState = { host: "unknown", bridge: "unknown" };
let rejected = 0;
const tabs = new Set<chrome.runtime.Port>();
/** What each recording tab shows in the popup: its title, then its progress line. */
const recordings = new Map<chrome.runtime.Port, string>();

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

chrome.runtime.onConnect.addListener((port) => {
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
  });
  port.onDisconnect.addListener(() => {
    tabs.delete(port);
    recordings.delete(port);
  });
});

chrome.runtime.onMessage.addListener((request: PopupRequest, _sender, respond) => {
  if (request.kind === "check-host") ensureHost();
  const state: PopupState = { connection, recordings: [...recordings.values()], rejected };
  respond(state);
  return false;
});

chrome.runtime.onInstalled.addListener(() => {
  void hasTeamsAccess(chrome.permissions, chrome.runtime.getManifest()).then((granted) => {
    if (!granted) void chrome.tabs.create({ url: chrome.runtime.getURL("popup.html") });
  });
});
