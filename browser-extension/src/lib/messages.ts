/**
 * The messages the extension's parts exchange (Issue #102):
 *
 * - page hook (page's own JavaScript world) ⇄ content script: `window.postMessage`, tagged with
 *   `HOOK_SOURCE` or `CONTENT_SOURCE` so other page messages are ignored;
 * - content script ⇄ background: a `chrome.runtime` port named `PORT_NAME` per Teams tab;
 * - background ⇄ native host: `HostMessage`, the JSON schema of
 *   `core/adapters/network/browser_host/extension_message.rs`.
 */

/** Tags messages the page hook posts. */
export const HOOK_SOURCE = "note-taker-hook";
/** Tags messages the content script posts to the page hook. */
export const CONTENT_SOURCE = "note-taker-content";
/** The content script's port to the background. */
export const PORT_NAME = "note-taker-tab";
/** The native messaging host's name. */
export const HOST_NAME = "com.note_taker.browser_bridge";

/** Which audio track a piece of audio belongs to. */
export type AudioTrack = "microphone" | "remote";

/** From the page hook to the content script. */
export type HookMessage =
  | { source: typeof HOOK_SOURCE; kind: "audio"; track: AudioTrack; samples: ArrayBuffer }
  | { source: typeof HOOK_SOURCE; kind: "transcript-file"; text: string; contentType: string };

/** From the content script to the page hook. */
export type HookCommand =
  | { source: typeof CONTENT_SOURCE; command: "start-audio" }
  | { source: typeof CONTENT_SOURCE; command: "stop-audio" };

/** A participant as the host expects it. */
export interface HostParticipant {
  id: string;
  name: string;
}

/** From the background to the native host. */
export type HostMessage =
  | { type: "meeting"; meetingId: string; title: string; startTimeMs: number; participants: HostParticipant[] }
  | { type: "audio"; track: AudioTrack; participant?: HostParticipant; timestampMs: number; pcm: string }
  | { type: "slide"; timestampMs: number; image: string }
  | {
      type: "document";
      kind: "teams_transcript" | "teams_live_transcript" | "teams_captions";
      mediaType: "text/vtt" | "text/plain";
      text: string;
      timestampMs: number;
      meetingId?: string;
    }
  | { type: "ended"; meetingId: string };

/** From the native host to the background. */
export type HostReply =
  | { type: "status"; bridge: "connected" | "unavailable" }
  | { type: "rejected"; reason: string };

/** The state the background reports to tabs and the popup. */
export interface ConnectionState {
  /** Whether the native host runs (`missing` when the browser could not start it). */
  host: "connected" | "missing" | "unknown";
  /** Whether the host reaches the Note Taker app. */
  bridge: "connected" | "unavailable" | "unknown";
  /** The browser's reason when the host could not start. */
  hostError?: string;
}

/** From the content script to the background. */
export type TabMessage =
  | { kind: "host"; message: HostMessage }
  | { kind: "recording"; recording: boolean; title: string };

/** From the background to a content script. */
export type BackgroundMessage = { kind: "connection"; state: ConnectionState };
