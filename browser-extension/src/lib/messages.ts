/**
 * The messages the extension's parts exchange (Issue #102):
 *
 * - page hook (page's own JavaScript world) ⇄ content script: `window.postMessage`, tagged with
 *   `HOOK_SOURCE` or `CONTENT_SOURCE` so other page messages are ignored;
 * - content script ⇄ background: a `chrome.runtime` port named `PORT_NAME` per Teams tab;
 * - recap frame script ⇄ background: a port named `RECAP_FRAME_PORT_NAME` per SharePoint frame a
 *   Teams page opened; the background relays between it and its tab's content script;
 * - background ⇄ native host: `HostMessage`, the JSON schema of
 *   `core/adapters/network/browser_host/extension_message.rs`.
 */
import { msg } from "./i18n";

/** Tags messages the page hook posts. */
export const HOOK_SOURCE = "note-taker-hook";
/** Tags messages the content script posts to the page hook. */
export const CONTENT_SOURCE = "note-taker-content";
/** The content script's port to the background. */
export const PORT_NAME = "note-taker-tab";
/** The recap frame script's port to the background. */
export const RECAP_FRAME_PORT_NAME = "note-taker-recap-frame";
/** The native messaging host's name. */
export const HOST_NAME = "com.note_taker.browser_bridge";

/** Which audio track a piece of audio belongs to. */
export type AudioTrack = "microphone" | "remote";

/** From the page hook to the content script. */
export type HookMessage =
  | { source: typeof HOOK_SOURCE; kind: "audio"; track: AudioTrack; samples: ArrayBuffer }
  | { source: typeof HOOK_SOURCE; kind: "transcript-file"; text: string; contentType: string }
  /** Answer to `analyse-share`: whether a share is shown and its frame's `signatureOf`. */
  | { source: typeof HOOK_SOURCE; kind: "share-frame"; present: boolean; signature: number[] }
  /** Answer to `encode-slide`: the last analysed frame cropped to the slide, base64 JPEG ("" when none). */
  | { source: typeof HOOK_SOURCE; kind: "slide-image"; image: string };

/** From the content script to the page hook. */
export type HookCommand =
  | { source: typeof CONTENT_SOURCE; command: "start-audio" }
  | { source: typeof CONTENT_SOURCE; command: "stop-audio" }
  | { source: typeof CONTENT_SOURCE; command: "analyse-share" }
  | { source: typeof CONTENT_SOURCE; command: "encode-slide" };

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
  /** Whether the host reaches the Koalino app. */
  bridge: "connected" | "unavailable" | "unknown";
  /** The browser's reason when the host could not start. */
  hostError?: string;
}

/** What a tab has sent to Koalino for the meeting it records, for the popup. */
export interface RecordingProgress {
  title: string;
  audioChunks: number;
  slides: number;
  captions: number;
  /** The last thing that failed, as "<step>: <error name>"; never page content. */
  problem: string | null;
}

/** From the content script to the background. */
export type TabMessage =
  | { kind: "host"; message: HostMessage }
  /** Recording started or stopped in the tab; `meetingId` is the Koalino meeting's (#102). */
  | {
      kind: "recording";
      recording: boolean;
      title: string;
      meetingId: string;
      /** The call was left in the page: end the meeting unless it is rejoined in time. */
      interrupted?: boolean;
    }
  | { kind: "progress"; progress: RecordingProgress }
  /** Asks the tab's recap frames to download the recap transcript (`downloadRecapVtt`). */
  | { kind: "recap-download" }
  /** What happened to the last Teams transcript fetch, for the popup (titles and times only). */
  | { kind: "recap-status"; status: string };

/**
 * The popup's line for a recording in progress.
 *
 * Inputs: The progress. Outputs: "<title>: audio chunks N, slides N, captions N" in the browser's
 * language (counts after their label, so no plural forms are needed), with the last problem in
 * brackets. Side effects: None.
 */
export function describeProgress(progress: RecordingProgress): string {
  const line = msg("recordingProgress", {
    title: progress.title || msg("teamsMeeting"),
    audio: progress.audioChunks,
    slides: progress.slides,
    captions: progress.captions,
  });
  return progress.problem ? msg("recordingProblem", { line, problem: progress.problem }) : line;
}

/** From the background to a content script. */
export type BackgroundMessage =
  | { kind: "connection"; state: ConnectionState }
  /** A WebVTT transcript the page hook copied in one of the tab's recap frames. */
  | { kind: "recap-transcript"; text: string };

/** From a recap frame script to the background: a WebVTT transcript the frame's hook copied. */
export type RecapFrameMessage = { kind: "recap-transcript"; text: string };

/** From the background to a recap frame script. */
export type RecapFrameCommand = { kind: "recap-download" };
