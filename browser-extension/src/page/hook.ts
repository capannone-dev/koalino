/**
 * The page hook (Issue #102): runs in a call page's own JavaScript world at document start,
 * because only there can the extension see the microphone stream the page opens and the remote
 * audio it plays, and the transcript file the page downloads.
 *
 * It runs on Teams (below), and since #174 on any other call page: the known meeting sites, every
 * site once the owner allows it, or the tab the owner records with "Record this tab" (injected
 * then, so it may arrive mid-call). There it reads what any WebRTC call shows:
 *
 * - Call: `report-call` answers with `callActivity`: the page's call connections, and the
 *   microphone and camera it opened, so the content script can offer to record.
 * - Microphone: as on Teams, the audio track the page sends on its call connection.
 * - Remote audio: every audio track the call connections receive, mixed (Google Meet plays the
 *   loudest speakers as separate tracks); without any, the media elements, as on Teams.
 * - Screen share: the screen the owner shares (`getDisplayMedia`), else the video that looks like
 *   a still screen (`ShareTracker`); then cropped to the slide as on Teams.
 *
 * On Teams:
 * - Microphone: the audio track Teams sends on its call connection (`sentMicrophone`, #157), which
 *   is found even when Teams opened the microphone before this hook ran. Before a call connection
 *   is seen, the audio tracks `getUserMedia` opened stand in. A muted microphone yields silence or
 *   no track, which the content script does not send.
 * - Remote audio: the `<audio>` element Teams plays a call through (one track with every remote
 *   participant mixed) is tapped with an `AudioContext`; playback is unaffected. In Chrome that
 *   element is never added to the page, so every element given a stream is noted (#160).
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
import { callActivity, remoteAudioTracks } from "../lib/callSignal";
import { resampleTo16k } from "../lib/pcm";
import { ShareCandidate, ShareTracker, screenContentStats } from "../lib/screenShare";
import { findSlideBox, signatureOf } from "../lib/slides";
import { claimHookGeneration, observeConnections, sentMicrophone } from "../lib/sentMicrophone";
import { embeddedInTeams } from "../lib/siteAccess";
import { serviceForHost } from "../lib/sites";
import { findShareVideo } from "../lib/teamsDom";
import { observePlayedStreams, remoteCallStream } from "../lib/remoteAudio";
import { AudioTrack, CONTENT_SOURCE, HOOK_SOURCE, HookCommand, HookMessage } from "../lib/messages";

/** Whether this page is Teams itself (the manifest's Teams sites), not a SharePoint frame. */
const onTeams = serviceForHost(window.location.hostname)?.id === "teams";
/** Whether this page is SharePoint, where the hook acts only in a recap frame Teams opened. */
const onSharePoint = /(^|\.)sharepoint\.com$/.test(window.location.hostname);
/** Whether this page is a call page of any other site (#174). */
const onOtherCallPage = !onTeams && !onSharePoint;

/** Samples per audio callback (about 85 ms at 48 kHz). */
const PROCESSOR_FRAMES = 4096;
/** How often the tapped elements and tracks are re-checked. */
const RESCAN_MS = 1_000;

/**
 * Whether this copy of the hook is the newest in the page. An extension update injects a new copy
 * while the old one stays; the old one then stops tapping and ignores commands (#157).
 */
const isNewestHook = claimHookGeneration(window);

/** Microphone tracks the page opened, newest last. */
const microphoneTracks: MediaStreamTrack[] = [];
/** Camera tracks the page opened (#174). */
const cameraTracks: MediaStreamTrack[] = [];
/** Screen tracks the page shares with `getDisplayMedia` (#174). */
const displayTracks: MediaStreamTrack[] = [];

/** Adds a track to `list` and removes it once it ends. */
function keepWhileLive(list: MediaStreamTrack[], track: MediaStreamTrack): void {
  list.push(track);
  track.addEventListener("ended", () => {
    const index = list.indexOf(track);
    if (index >= 0) list.splice(index, 1);
  });
}

/** Most media elements given a stream that are remembered (Teams uses one or two per call). */
const MAXIMUM_PLAYED_ELEMENTS = 16;

/**
 * Media elements Teams gave a stream, newest last, held weakly: in Chrome the call's `<audio>`
 * element is not in the page, so it cannot be found there (#160).
 */
const playedElements: WeakRef<HTMLMediaElement>[] = [];

/** Notes a media element Teams gave a stream. */
function rememberPlayed(element: HTMLMediaElement): void {
  playedElements.push(new WeakRef(element));
  if (playedElements.length > MAXIMUM_PLAYED_ELEMENTS) playedElements.shift();
}

/** The remembered media elements still alive, newest first (dead ones are dropped). */
function livePlayedElements(): HTMLMediaElement[] {
  const alive: HTMLMediaElement[] = [];
  for (let index = playedElements.length - 1; index >= 0; index -= 1) {
    const element = playedElements[index]?.deref();
    if (element) alive.push(element);
    else playedElements.splice(index, 1);
  }
  return alive;
}

/** Call connections Teams used (closed ones are dropped on each rescan). */
const callConnections = new Set<RTCPeerConnection>();

/** Posts a message to the content script (same window, same origin). */
function post(message: HookMessage, transfer: Transferable[] = []): void {
  window.postMessage(message, window.location.origin, transfer);
}

/** Wraps `getUserMedia` to remember microphone and camera tracks. */
function hookGetUserMedia(): void {
  const mediaDevices = navigator.mediaDevices;
  if (!mediaDevices?.getUserMedia) return;
  const original = mediaDevices.getUserMedia.bind(mediaDevices);
  mediaDevices.getUserMedia = async (constraints?: MediaStreamConstraints) => {
    const stream = await original(constraints);
    for (const track of stream.getAudioTracks()) keepWhileLive(microphoneTracks, track);
    for (const track of stream.getVideoTracks()) keepWhileLive(cameraTracks, track);
    return stream;
  };
}

/** Wraps `getDisplayMedia` to remember the screens the owner shares (#174). */
function hookGetDisplayMedia(): void {
  const mediaDevices = navigator.mediaDevices;
  if (!mediaDevices?.getDisplayMedia) return;
  const original = mediaDevices.getDisplayMedia.bind(mediaDevices);
  mediaDevices.getDisplayMedia = async (options?: DisplayMediaStreamOptions) => {
    const stream = await original(options);
    for (const track of stream.getVideoTracks()) keepWhileLive(displayTracks, track);
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

/** One tapped track: its streams' source nodes, mixed into one processor. */
interface Tap {
  /** The tapped audio tracks' IDs, sorted and joined: a tap is redone when they change. */
  key: string;
  sources: MediaStreamAudioSourceNode[];
  processor: ScriptProcessorNode;
  pending: Int16Array[];
  pendingLength: number;
}

/** The key of a set of streams: their audio tracks' IDs. */
function tapKey(streams: readonly MediaStream[]): string {
  return streams
    .flatMap((stream) => stream.getAudioTracks().map((track) => track.id))
    .sort()
    .join(",");
}

let context: AudioContext | null = null;
let silentSink: GainNode | null = null;
const taps = new Map<AudioTrack, Tap>();
let rescanTimer: number | undefined;

/**
 * Starts tapping `streams` as `track`, mixed into one, replacing an earlier tap of that track
 * unless it taps the same audio tracks.
 */
function tap(track: AudioTrack, streams: readonly MediaStream[]): void {
  if (!context || !silentSink) return;
  const key = tapKey(streams);
  const existing = taps.get(track);
  if (existing?.key === key) return;
  if (existing) untap(track);
  if (key === "") return;
  const audioContext = context;
  const sources = streams.map((stream) => audioContext.createMediaStreamSource(stream));
  const processor = context.createScriptProcessor(PROCESSOR_FRAMES, 1, 1);
  const state: Tap = { key, sources, processor, pending: [], pendingLength: 0 };
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
  // Sources connected to one input are summed: the remote speakers come out as one mix.
  for (const source of sources) source.connect(processor);
  processor.connect(silentSink);
  taps.set(track, state);
}

/** Stops tapping `track`. */
function untap(track: AudioTrack): void {
  const existing = taps.get(track);
  if (!existing) return;
  existing.processor.onaudioprocess = null;
  for (const source of existing.sources) source.disconnect();
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
  } else if (taps.get("microphone")?.key !== microphone.id) {
    tap("microphone", [new MediaStream([microphone])]);
  }
  // Elsewhere than Teams: every audio track the call receives, mixed (#174).
  const received = onOtherCallPage ? remoteAudioTracks<MediaStreamTrack>(callConnections) : [];
  if (received.length > 0) {
    tap("remote", received.map((track) => new MediaStream([track])));
    return;
  }
  const remote = remoteCallStream<MediaStream>(
    [...document.querySelectorAll("audio"), ...livePlayedElements()],
    (track) => track === microphone || microphoneTracks.includes(track as MediaStreamTrack),
  );
  if (remote) tap("remote", [remote]);
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

/** Candidate frames are drawn this small to judge them (text strokes still make edges). */
const CANDIDATE_WIDTH = 160;
const CANDIDATE_HEIGHT = 90;
/** Chooses the shared screen among the page's videos, elsewhere than Teams (#174). */
const shareTracker = new ShareTracker<HTMLVideoElement>();
/** The canvas candidate frames are drawn on. */
let candidateCanvas: HTMLCanvasElement | null = null;
/** A detached video showing the screen the owner shares, which the page may never show. */
let ownShareVideo: HTMLVideoElement | null = null;

/** Whether a value is a stream with a live video track. */
function liveVideoStream(value: unknown): value is MediaStream {
  return value instanceof MediaStream && value.getVideoTracks().some((track) => track.readyState === "live");
}

/** The detached video for the newest screen the owner shares, or null when none is live. */
function currentOwnShareVideo(): HTMLVideoElement | null {
  const shared = [...displayTracks].reverse().find((track) => track.readyState === "live");
  if (!shared) {
    if (ownShareVideo) ownShareVideo.srcObject = null;
    ownShareVideo = null;
    return null;
  }
  const shown = ownShareVideo?.srcObject;
  if (ownShareVideo && shown instanceof MediaStream && shown.getVideoTracks()[0] === shared) return ownShareVideo;
  ownShareVideo = document.createElement("video");
  ownShareVideo.muted = true;
  ownShareVideo.playsInline = true;
  ownShareVideo.srcObject = new MediaStream([shared]);
  void ownShareVideo.play().catch(() => undefined);
  return ownShareVideo;
}

/** The camera tracks the owner sends or shows: opened with `getUserMedia`, or sent on a call. */
function localCameraTracks(): Set<MediaStreamTrack> {
  const local = new Set<MediaStreamTrack>(cameraTracks);
  for (const connection of callConnections) {
    try {
      for (const sender of connection.getSenders()) {
        const track = sender.track;
        if (track?.kind === "video" && !displayTracks.includes(track)) local.add(track);
      }
    } catch {
      // A closed connection: nothing to add.
    }
  }
  return local;
}

/**
 * Measures every video the page plays from a stream and chooses the shared screen (#174).
 *
 * Outputs: The video showing the share, or null. Side effects: Draws on `candidateCanvas`.
 */
function findAnyShareVideo(): HTMLVideoElement | null {
  const own = currentOwnShareVideo();
  const videos = new Set<HTMLVideoElement>(
    [...document.querySelectorAll("video"), ...livePlayedElements()].filter(
      (element): element is HTMLVideoElement => element instanceof HTMLVideoElement,
    ),
  );
  if (own) videos.add(own);
  candidateCanvas ??= document.createElement("canvas");
  candidateCanvas.width = CANDIDATE_WIDTH;
  candidateCanvas.height = CANDIDATE_HEIGHT;
  const canvasContext = candidateCanvas.getContext("2d", { willReadFrequently: true });
  if (!canvasContext) return null;
  const localCameras = localCameraTracks();
  const candidates: ShareCandidate<HTMLVideoElement>[] = [];
  for (const video of videos) {
    const stream = video.srcObject;
    if (!liveVideoStream(stream) || video.videoWidth === 0 || video.videoHeight === 0) continue;
    const tracks = stream.getVideoTracks();
    let stats = null;
    let signature = new Float32Array();
    try {
      canvasContext.drawImage(video, 0, 0, CANDIDATE_WIDTH, CANDIDATE_HEIGHT);
      const frame = canvasContext.getImageData(0, 0, CANDIDATE_WIDTH, CANDIDATE_HEIGHT);
      const image = { width: CANDIDATE_WIDTH, height: CANDIDATE_HEIGHT, data: frame.data };
      stats = screenContentStats(image);
      signature = signatureOf(image);
    } catch {
      // A video that cannot be read is no candidate this round.
    }
    candidates.push({
      video,
      width: video.videoWidth,
      height: video.videoHeight,
      ownShare: video === own || tracks.some((track) => displayTracks.includes(track)),
      localCamera: tracks.some((track) => localCameras.has(track)),
      stats,
      signature,
    });
  }
  return shareTracker.choose(candidates);
}

/**
 * Draws the shown screen share and posts whether one is shown and its frame's signature.
 * Side effects: Redraws `frameCanvas`; posts a `share-frame` message.
 */
function analyseShare(): void {
  const video = onTeams ? findShareVideo(document) : findAnyShareVideo();
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

/** Posts what the page does with calls and media now (#174). */
function reportCall(): void {
  for (const connection of callConnections) {
    if (connection.connectionState === "closed") callConnections.delete(connection);
  }
  const activity = callActivity(callConnections, [...microphoneTracks, ...cameraTracks], displayTracks);
  post({ source: HOOK_SOURCE, kind: "call-activity", ...activity });
}

window.addEventListener("message", (event: MessageEvent<HookCommand>) => {
  if (onSharePoint || event.source !== window || event.data?.source !== CONTENT_SOURCE) return;
  if (!isNewestHook()) {
    // A newer copy of the hook answers; this one only lets go of its audio.
    stopAudio();
    return;
  }
  if (event.data.command === "start-audio") startAudio();
  if (event.data.command === "stop-audio") stopAudio();
  if (event.data.command === "report-call") reportCall();
  try {
    if (event.data.command === "analyse-share") analyseShare();
    if (event.data.command === "encode-slide") encodeSlide();
  } catch {
    // A tainted or vanished video: report nothing rather than disturb the page.
    if (event.data.command === "encode-slide") post({ source: HOOK_SOURCE, kind: "slide-image", image: "" });
  }
});

if (!onSharePoint) {
  hookGetUserMedia();
  hookGetDisplayMedia();
  observePlayedStreams(window.HTMLMediaElement?.prototype, rememberPlayed);
  observeConnections(window.RTCPeerConnection?.prototype, (connection) => callConnections.add(connection));
  if (onTeams) hookFetch();
} else if (embeddedInTeams(window.top === window, document.referrer)) {
  // A recap frame: only its transcript download is copied.
  hookFetch();
}
