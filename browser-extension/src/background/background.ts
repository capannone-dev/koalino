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
 * A tab whose port closes while it records may be reloading: the meeting is ended in Koalino only
 * if no tab resumes it within two minutes (`InterruptedRecordings`, #102).
 *
 * It also relays between a tab and its recap frames (the SharePoint frame Teams shows a recap
 * transcript in): the tab asks them to download the transcript, and they pass back the WebVTT
 * their page hook copied.
 *
 * After it is installed without access to the meeting sites (Firefox, when loaded by hand), it
 * opens its settings page in a tab, where the owner can allow it.
 *
 * Since #174 it also records any tab the owner asks for from the popup ("Record this tab"),
 * bringing the scripts there first, keeps the scripts registered on every website while the owner
 * wants that ("Watch every website for calls"), and marks each recording tab with a REC badge.
 */
import { msg } from "../lib/i18n";
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
import { ANY_SITE_PATTERNS, ensureTabScripts, syncAnySiteScripts } from "../lib/anySite";
import { InterruptedRecordings } from "../lib/interruptions";
import { hasTeamsAccess } from "../lib/siteAccess";
import { knownSitePatterns } from "../lib/sites";
import { loadSettings, settingsChanged } from "../lib/storage";
import { TabCommand, TabCommandReply } from "../lib/messages";

/** Requests the popup sends. */
export type PopupRequest =
  | { kind: "get-state" }
  | { kind: "check-host" }
  /** "Record this tab" or its Stop (#174). */
  | { kind: "record-tab"; tabId: number }
  | { kind: "stop-tab"; tabId: number };

/** The answer to `record-tab` and `stop-tab`. */
export interface TabRecordingReply {
  /** Whether the tab records now; false with `failed` when the browser refused the tab. */
  recording: boolean;
  failed?: boolean;
}

/** What the popup shows. */
export interface PopupState {
  connection: ConnectionState;
  recordings: string[];
  /** Each tab's last Teams transcript fetch status. */
  recapStatuses: string[];
  rejected: number;
  /** The tabs recording now (#174), so the popup offers Stop on its tab. */
  recordingTabIds: number[];
}

let hostPort: chrome.runtime.Port | null = null;
let connection: ConnectionState = { host: "unknown", bridge: "unknown" };
let rejected = 0;
const tabs = new Set<chrome.runtime.Port>();
/** What each recording tab shows in the popup: its title, then its progress line. */
const recordings = new Map<chrome.runtime.Port, string>();
/** The Koalino meeting each recording tab records. */
const recordingMeetings = new Map<chrome.runtime.Port, string>();
/** Meetings whose tab went away mid-call, ended unless resumed (#102). */
const interruptions = new InterruptedRecordings((meetingId) => sendToHost({ type: "ended", meetingId }), {
  setTimeout: (run, delayMs) => self.setTimeout(run, delayMs),
  clearTimeout: (timerId) => self.clearTimeout(timerId),
});
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
      if (reply.type === "status") {
        // Connected: the app's answer (none for Auto). Unavailable: what it said last (#141).
        const language = reply.bridge === "connected" ? reply.language : connection.language;
        setConnection({ host: "connected", bridge: reply.bridge, ...(language ? { language } : {}) });
      }
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
      markTab(port.sender?.tab?.id, message.recording);
      if (message.recording) {
        recordings.set(port, message.title || msg("teamsMeeting"));
        recordingMeetings.set(port, message.meetingId);
        interruptions.resumed(message.meetingId);
      } else {
        recordings.delete(port);
        recordingMeetings.delete(port);
        if (message.interrupted) interruptions.interrupted(message.meetingId);
      }
    }
    if (message.kind === "progress" && recordings.has(port)) recordings.set(port, describeProgress(message.progress));
    if (message.kind === "recap-download") requestRecapDownload(port.sender?.tab?.id);
    if (message.kind === "recap-status") recapStatuses.set(port, message.status);
  });
  port.onDisconnect.addListener(() => {
    if (recordings.has(port)) markTab(port.sender?.tab?.id, false);
    tabs.delete(port);
    recordings.delete(port);
    recapStatuses.delete(port);
    const interruptedMeeting = recordingMeetings.get(port);
    recordingMeetings.delete(port);
    if (interruptedMeeting !== undefined) interruptions.interrupted(interruptedMeeting);
  });
});

/**
 * Shows or clears a tab's REC badge.
 *
 * Inputs: The tab (none: nothing happens) and whether it records. Side effects: Sets the badge.
 */
function markTab(tabId: number | undefined, recording: boolean): void {
  if (tabId === undefined) return;
  void chrome.action.setBadgeText({ tabId, text: recording ? "REC" : "" }).catch(() => undefined);
  if (recording) void chrome.action.setBadgeBackgroundColor({ tabId, color: "#b91c1c" }).catch(() => undefined);
}

/**
 * Records a tab the owner chose, or stops it (#174).
 *
 * Inputs: The request. Outputs: Whether the tab records now; `failed` when the browser refused
 * the tab (a browser page, a store page, or no access).
 * Side effects: Injects the scripts where missing, then tells the tab's content script.
 */
async function recordTab(request: { kind: "record-tab" | "stop-tab"; tabId: number }): Promise<TabRecordingReply> {
  try {
    if (request.kind === "record-tab") await ensureTabScripts(chrome.scripting, request.tabId);
    const command: TabCommand = { kind: request.kind };
    const reply = (await chrome.tabs.sendMessage(request.tabId, command)) as TabCommandReply | undefined;
    return { recording: reply?.recording === true };
  } catch {
    return { recording: false, failed: request.kind === "record-tab" };
  }
}

/** The tabs recording now. */
function recordingTabIds(): number[] {
  return [...recordings.keys()].flatMap((port) => (port.sender?.tab?.id === undefined ? [] : [port.sender.tab.id]));
}

chrome.runtime.onMessage.addListener((request: PopupRequest, _sender, respond) => {
  if (request.kind === "record-tab" || request.kind === "stop-tab") {
    void recordTab(request).then(respond);
    return true;
  }
  if (request.kind === "check-host") ensureHost();
  const state: PopupState = {
    connection,
    recordings: [...recordings.values()],
    recapStatuses: [...recapStatuses.values()],
    rejected,
    recordingTabIds: recordingTabIds(),
  };
  respond(state);
  return false;
});

/**
 * Registers or removes the scripts for every website to match the owner's choice (#174).
 * Side effects: Reads the settings and the granted permissions; changes the registrations.
 */
async function syncAnySite(): Promise<void> {
  try {
    const [settings, allowed] = await Promise.all([
      loadSettings(),
      chrome.permissions.contains({ origins: ANY_SITE_PATTERNS }),
    ]);
    await syncAnySiteScripts(chrome.scripting, {
      wanted: settings.watchAllSites,
      allowed,
      knownSites: knownSitePatterns(),
    });
  } catch {
    // A browser without registered scripts: "Record this tab" still works.
  }
}

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && settingsChanged(changes)) void syncAnySite();
});
chrome.permissions.onAdded.addListener(() => void syncAnySite());
chrome.permissions.onRemoved.addListener(() => void syncAnySite());
void syncAnySite();

chrome.runtime.onInstalled.addListener(() => {
  void hasTeamsAccess(chrome.permissions, chrome.runtime.getManifest()).then((granted) => {
    if (!granted) void chrome.tabs.create({ url: chrome.runtime.getURL("popup.html") });
  });
});
