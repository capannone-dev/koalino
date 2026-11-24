/**
 * The page hook (Issue #102): runs in the Teams page's own JavaScript world at document start,
 * because only there can the extension see the microphone stream Teams opens and the remote
 * audio Teams plays, and the transcript file the page downloads.
 *
 * - Microphone: the audio track Teams sends on its call connection (`sentMicrophone`, #157), which
 *   is found even when Teams opened the microphone before this hook ran. Before a call connection
 *   is seen, the audio tracks `getUserMedia` opened stand in. A muted microphone yields silence or
 *   no track, which the content script does not send.
 * - Remote audio: the `<audio>` elements Teams plays a call through (one track with every remote
 *   participant mixed) are tapped with an `AudioContext`; playback is unaffected.
 * - Teams transcript: responses of the page's own transcript requests (`/transcripts/…`) are
 *   copied; the extension never makes such requests itself and never uses the page's tokens.
 *   In the recap's SharePoint frame (opened by Teams) the hook does only this, for the WebVTT the
 *   frame's Download → "Download as .vtt" fetches; on any other SharePoint page it does nothing.
 *
 * - Screen share: on `analyse-share` the shared screen's video is drawn on a canvas here and only
 *   its small signature is posted; on `encode-slide` that frame is cropped to the slide and posted
 *   as a base64 JPEG. Pixels are read in this world because in Firefox a content script reading a
 *   page video's pixels, or awaiting the page's promises, either crawls through per-byte wrappers
 *   (it froze Teams in the #102 live test) or is denied; plain strings and numbers cross cleanly.
 *
 * Audio is tapped only between the content script's `start-audio` and `stop-audio`, converted to
 * 16 kHz PCM, and posted to the content script about once a second. Nothing is logged.
 */
import { resampleTo16k } from "../lib/pcm";
import { findSlideBox, signatureOf } from "../lib/slides";
import { claimHookGeneration, observeConnections, sentMicrophone } from "../lib/sentMicrophone";
import { embeddedInTeams } from "../lib/siteAccess";
import { findShareVideo } from "../lib/teamsDom";
import { AudioTrack, CONTENT_SOURCE, HOOK_SOURCE, HookCommand, HookMessage } from "../lib/messages";

/** Whether this page is Teams itself (the manifest's Teams sites), not a SharePoint frame. */
const onTeams = /^teams\.(microsoft\.com|cloud\.microsoft|live\.com)$/.test(window.location.hostname);

/** Samples per audio callback (about 85 ms at 48 kHz). */
const PROCESSOR_FRAMES = 4096;
/** How often the tapped elements and tracks are re-checked. */
const RESCAN_MS = 1_000;

/**
 * Whether this copy of the hook is the newest in the page. An extension update injects a new copy
 * while the old one stays; the old one then stops tapping and ignores commands (#157).
 */
const isNewestHook = claimHookGeneration(window);

/** Microphone tracks Teams opened, newest last. */
const microphoneTracks: MediaStreamTrack[] = [];

/** Call connections Teams used (closed ones are dropped on each rescan). */
const callConnections = new Set<RTCPeerConnection>();

/** Posts a message to the content script (same window, same origin). */
function post(message: HookMessage, transfer: Transferable[] = []): void {
  window.postMessage(message, window.location.origin, transfer);
}

/** Wraps `getUserMedia` to remember microphone tracks. */
function hookGetUserMedia(): void {
  const mediaDevices = navigator.mediaDevices;
  if (!mediaDevices?.getUserMedia) return;
  const original = mediaDevices.getUserMedia.bind(mediaDevices);
  mediaDevices.getUserMedia = async (constraints?: MediaStreamConstraints) => {
    const stream = await original(constraints);
    for (const track of stream.getAudioTracks()) {
      microphoneTracks.push(track);
      track.addEventListener("ended", () => {
        const index = microphoneTracks.indexOf(track);
        if (index >= 0) microphoneTracks.splice(index, 1);
      });
    }
    return stream;
  };
}

/** Wraps `fetch` to copy the page's own transcript downloads. */
function hookFetch(): void {
  const original = window.fetch.bind(window);
  window.fetch = async (...args: Parameters<typeof fetch>) => {
    const response = await original(...args);
    try {
      const request = args[0];
      const url = typeof request === "string" ? request : request instanceof URL ? request.href : request.url;
      if (/\/transcripts?\//i.test(url) && response.ok) {
        const contentType = response.headers.get("content-type") ?? "";
        void response
          .clone()
          .text()
          .then((text) => post({ source: HOOK_SOURCE, kind: "transcript-file", text, contentType }))
          .catch(() => undefined);
      }
    } catch {
      // Never disturb the page's own request.
    }
    return response;
  };
}

/** One tapped stream: its source node and processor. */
interface Tap {
  stream: MediaStream;
  source: MediaStreamAudioSourceNode;
  processor: ScriptProcessorNode;
  pending: Int16Array[];
  pendingLength: number;
}

let context: AudioContext | null = null;
let silentSink: GainNode | null = null;
const taps = new Map<AudioTrack, Tap>();
let rescanTimer: number | undefined;

/** Starts tapping `stream` as `track`, replacing an earlier tap of that track. */
function tap(track: AudioTrack, stream: MediaStream): void {
  if (!context || !silentSink) return;
  const existing = taps.get(track);
  if (existing?.stream === stream) return;
  if (existing) untap(track);
  const source = context.createMediaStreamSource(stream);
  const processor = context.createScriptProcessor(PROCESSOR_FRAMES, 1, 1);
  const state: Tap = { stream, source, processor, pending: [], pendingLength: 0 };
  const sampleRate = context.sampleRate;
  processor.onaudioprocess = (event) => {
    if (!isNewestHook()) {
      stopAudio();
      return;
    }
    const samples = resampleTo16k(event.inputBuffer.getChannelData(0), sampleRate);
    state.pending.push(samples);
    state.pendingLength += samples.length;
    if (state.pendingLength >= 16_000) {
      const merged = new Int16Array(state.pendingLength);
      let offset = 0;
      for (const part of state.pending) {
        merged.set(part, offset);
        offset += part.length;
      }
      state.pending = [];
      state.pendingLength = 0;
      post({ source: HOOK_SOURCE, kind: "audio", track, samples: merged.buffer }, [merged.buffer]);
    }
  };
  source.connect(processor);
  processor.connect(silentSink);
  taps.set(track, state);
}

/** Stops tapping `track`. */
function untap(track: AudioTrack): void {
  const existing = taps.get(track);
  if (!existing) return;
  existing.processor.onaudioprocess = null;
  existing.source.disconnect();
  existing.processor.disconnect();
  taps.delete(track);
}

/**
 * The microphone track to tap: the one Teams sends, else the newest live `getUserMedia` track
 * while no call connection sends audio; null when Teams muted it or none is open.
 */
function currentMicrophone(): MediaStreamTrack | null {
  for (const connection of callConnections) {
    if (connection.connectionState === "closed") callConnections.delete(connection);
  }
  const sent = sentMicrophone<MediaStreamTrack>(callConnections);
  if (sent.kind === "track") return sent.track;
  if (sent.kind === "muted") return null;
  return [...microphoneTracks].reverse().find((track) => track.readyState === "live") ?? null;
}

/** Finds the current microphone and remote streams and taps them. */
function rescan(): void {
  const microphone = currentMicrophone();
  if (!microphone) {
    untap("microphone");
  } else {
    const current = taps.get("microphone");
    if (!current || current.stream.getAudioTracks()[0] !== microphone) {
      tap("microphone", new MediaStream([microphone]));
    }
  }
  const remote = [...document.querySelectorAll("audio")]
    .map((element) => element.srcObject)
    .find(
      (stream): stream is MediaStream =>
        stream instanceof MediaStream &&
        stream
          .getAudioTracks()
          .some((track) => track.readyState === "live" && track !== microphone && !microphoneTracks.includes(track)),
    );
  if (remote) tap("remote", remote);
}

/** Starts tapping (idempotent). */
function startAudio(): void {
  if (!context) {
    context = new AudioContext();
    silentSink = context.createGain();
    silentSink.gain.value = 0;
    silentSink.connect(context.destination);
  }
  void context.resume().catch(() => undefined);
  rescan();
  if (rescanTimer === undefined) rescanTimer = window.setInterval(rescan, RESCAN_MS);
}

/** Stops tapping and releases the audio context. */
function stopAudio(): void {
  if (rescanTimer !== undefined) window.clearInterval(rescanTimer);
  rescanTimer = undefined;
  untap("microphone");
  untap("remote");
  void context?.close().catch(() => undefined);
  context = null;
  silentSink = null;
}

/** Share frames wider than this are scaled down before analysis (keeps slides legible). */
const MAXIMUM_FRAME_WIDTH = 1920;
/** JPEG quality of a kept slide. */
const SLIDE_JPEG_QUALITY = 0.88;

/** The canvas the last analysed share frame was drawn on, reused between frames. */
let frameCanvas: HTMLCanvasElement | null = null;

/**
 * Draws the shown screen share and posts whether one is shown and its frame's signature.
 * Side effects: Redraws `frameCanvas`; posts a `share-frame` message.
 */
function analyseShare(): void {
  const video = findShareVideo(document);
  if (!video || video.videoWidth === 0 || video.videoHeight === 0) {
    post({ source: HOOK_SOURCE, kind: "share-frame", present: video !== null, signature: [] });
    return;
  }
  const scale = Math.min(1, MAXIMUM_FRAME_WIDTH / video.videoWidth);
  const width = Math.round(video.videoWidth * scale);
  const height = Math.round(video.videoHeight * scale);
  frameCanvas ??= document.createElement("canvas");
  if (frameCanvas.width !== width || frameCanvas.height !== height) {
    frameCanvas.width = width;
    frameCanvas.height = height;
  }
  const canvasContext = frameCanvas.getContext("2d", { willReadFrequently: true });
  if (!canvasContext) return;
  canvasContext.drawImage(video, 0, 0, width, height);
  const pixels = canvasContext.getImageData(0, 0, width, height);
  const signature = signatureOf({ width, height, data: pixels.data });
  post({ source: HOOK_SOURCE, kind: "share-frame", present: true, signature: Array.from(signature) });
}

/**
 * Crops the last analysed frame to the slide and posts it as base64 JPEG ("" when it cannot).
 * Side effects: Posts a `slide-image` message.
 */
function encodeSlide(): void {
  const source = frameCanvas;
  const sourceContext = source?.getContext("2d", { willReadFrequently: true });
  if (!source || !sourceContext || source.width === 0) {
    post({ source: HOOK_SOURCE, kind: "slide-image", image: "" });
    return;
  }
  const pixels = sourceContext.getImageData(0, 0, source.width, source.height);
  const box = findSlideBox({ width: source.width, height: source.height, data: pixels.data }) ?? {
    left: 0,
    top: 0,
    right: source.width,
    bottom: source.height,
  };
  const slide = document.createElement("canvas");
  slide.width = box.right - box.left;
  slide.height = box.bottom - box.top;
  slide.getContext("2d")?.drawImage(source, box.left, box.top, slide.width, slide.height, 0, 0, slide.width, slide.height);
  const dataUrl = slide.toDataURL("image/jpeg", SLIDE_JPEG_QUALITY);
  const comma = dataUrl.indexOf(",");
  post({ source: HOOK_SOURCE, kind: "slide-image", image: comma >= 0 ? dataUrl.slice(comma + 1) : "" });
}

window.addEventListener("message", (event: MessageEvent<HookCommand>) => {
  if (!onTeams || event.source !== window || event.data?.source !== CONTENT_SOURCE) return;
  if (!isNewestHook()) {
    // A newer copy of the hook answers; this one only lets go of its audio.
    stopAudio();
    return;
  }
  if (event.data.command === "start-audio") startAudio();
  if (event.data.command === "stop-audio") stopAudio();
  try {
    if (event.data.command === "analyse-share") analyseShare();
    if (event.data.command === "encode-slide") encodeSlide();
  } catch {
    // A tainted or vanished video: report nothing rather than disturb the page.
    if (event.data.command === "encode-slide") post({ source: HOOK_SOURCE, kind: "slide-image", image: "" });
  }
});

if (onTeams) {
  hookGetUserMedia();
  observeConnections(window.RTCPeerConnection?.prototype, (connection) => callConnections.add(connection));
  hookFetch();
} else if (embeddedInTeams(window.top === window, document.referrer)) {
  // A recap frame: only its transcript download is copied.
  hookFetch();
}
