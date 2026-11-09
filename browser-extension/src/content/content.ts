/**
 * The content script on Teams pages (Issue #102). It watches for a call, asks the owner whether
 * to record it (unless "Always for Teams" is set), and while recording:
 *
 * - tells the page hook to tap the microphone and the remote audio, and sends their chunks;
 * - turns on live captions and follows them, and follows the transcript panel;
 * - starts Teams transcription when the owner opted in;
 * - keeps screen-share slides, cropped to the slide.
 *
 * When the call ends it sends the captions and live transcript and ends the meeting. Outside
 * calls, when Teams posts a recap with a transcript for a meeting it recorded, it fetches the
 * transcript through the recap and sends it to the same Note Taker meeting.
 *
 * Everything goes to the background over a port, and from there to the native host. Nothing of
 * the meeting is logged.
 */
import { fetchRecapTranscript, startTeamsTranscription, turnOnCaptions } from "./automation";
import { askConsent, hideOverlay, showRecording } from "./banner";
import { MeetingRecorder } from "./recorder";
import {
  BackgroundMessage,
  CONTENT_SOURCE,
  ConnectionState,
  HOOK_SOURCE,
  HookCommand,
  HookMessage,
  HostMessage,
  PORT_NAME,
  TabMessage,
} from "../lib/messages";
import { RecordedMeeting, matchRecapToMeeting, newMeetingId, pendingTranscriptMeetings } from "../lib/meetings";
import { ExtensionSettings, DEFAULT_SETTINGS } from "../lib/settings";
import { RgbaImage } from "../lib/slides";
import { loadMeetings, loadSettings, saveMeeting, saveSettings, settingsChanged } from "../lib/storage";
import {
  findRecapCards,
  findShareVideo,
  isInCall,
  isTeamsTranscriptRunning,
  meetingTitleFromDocumentTitle,
  ownerName,
} from "../lib/teamsDom";

/** How often the page is checked. */
const TICK_MS = 1_000;
/** How long after recording starts captions are turned on (Teams' toolbar settles first). */
const CAPTIONS_DELAY_MS = 3_000;
/** How often recap cards are looked for outside calls. */
const RECAP_CHECK_MS = 5_000;
/** The largest share frame analysed, in pixels of width. */
const MAXIMUM_FRAME_WIDTH = 1920;

type CallState = "idle" | "asking" | "declined" | "recording";

let settings: ExtensionSettings = DEFAULT_SETTINGS;
let connection: ConnectionState = { host: "unknown", bridge: "unknown" };
let state: CallState = "idle";
let recorder: MeetingRecorder | null = null;
let record: RecordedMeeting | null = null;
let recordingStartedAt = 0;
let captionsAttempted = false;
let transcriptionAttempted = false;
let shareShown = false;
let lastRecapCheck = 0;
let recapFetch: { meeting: RecordedMeeting; arrived: () => void } | null = null;
const recapAttempted = new Set<string>();
let overlayStatus = "";

let port = connectPort();

/** Opens the port to the background (again, after the background restarted). */
function connectPort(): chrome.runtime.Port {
  const opened = chrome.runtime.connect({ name: PORT_NAME });
  opened.onMessage.addListener((message: BackgroundMessage) => {
    if (message.kind === "connection") {
      connection = message.state;
      refreshOverlay();
    }
  });
  opened.onDisconnect.addListener(() => {
    port = connectPort();
  });
  return opened;
}

/** Sends a message to the background. */
function toBackground(message: TabMessage): void {
  try {
    port.postMessage(message);
  } catch {
    port = connectPort();
    port.postMessage(message);
  }
}

/** Sends a message on to Note Taker. */
function toHost(message: HostMessage): void {
  toBackground({ kind: "host", message });
}

/** Sends a command to the page hook. */
function toHook(command: HookCommand["command"]): void {
  window.postMessage({ source: CONTENT_SOURCE, command } satisfies HookCommand, window.location.origin);
}

/** The recording overlay's status line. */
function statusLine(): string {
  if (connection.host === "missing") return "Note Taker helper not installed";
  if (connection.bridge === "unavailable") return "Note Taker app not running; data is queued";
  if (connection.bridge === "connected") return "sending to Note Taker";
  return "";
}

/** Redraws the recording overlay when its status changed. */
function refreshOverlay(): void {
  if (state !== "recording") return;
  const status = statusLine();
  if (status === overlayStatus) return;
  overlayStatus = status;
  showRecording(status, stopByOwner);
}

/** Starts recording the current call. */
function startRecording(): void {
  const startMs = Date.now();
  const meetingId = newMeetingId(startMs);
  const title = meetingTitleFromDocumentTitle(document.title);
  recorder = new MeetingRecorder(
    { meetingId, title, ownerName: ownerName(document) },
    { send: toHost, now: Date.now, root: document, encodeSlide },
  );
  record = {
    meetingId,
    title,
    startMs,
    endMs: null,
    teamsTranscriptSeen: false,
    transcriptFetched: false,
  };
  void saveMeeting(record);
  state = "recording";
  recordingStartedAt = startMs;
  captionsAttempted = false;
  transcriptionAttempted = false;
  shareShown = false;
  overlayStatus = "\u0000";
  toHook("start-audio");
  toBackground({ kind: "recording", recording: true, title });
  refreshOverlay();
}

/** Ends the recording (the call ended or the owner pressed Stop). */
function stopRecording(): void {
  if (!recorder || !record) return;
  toHook("stop-audio");
  recorder.stop();
  record = { ...record, endMs: Date.now(), teamsTranscriptSeen: recorder.sawTeamsTranscript() };
  void saveMeeting(record);
  toBackground({ kind: "recording", recording: false, title: record.title });
  recorder = null;
  hideOverlay();
}

/** The owner pressed Stop: no more recording until this call ends. */
function stopByOwner(): void {
  stopRecording();
  state = "declined";
}

/** Handles the owner's answer to the consent question. */
function onConsent(answer: "record" | "always" | "not-now"): void {
  if (state !== "asking") return;
  if (answer === "not-now") {
    state = "declined";
    return;
  }
  if (answer === "always") void saveSettings({ alwaysRecordTeams: true });
  startRecording();
}

/**
 * Draws the share video into an RGBA frame.
 * Outputs: The frame, or null when the video has no picture yet.
 */
function grabFrame(video: HTMLVideoElement): RgbaImage | null {
  if (video.videoWidth === 0 || video.videoHeight === 0) return null;
  const scale = Math.min(1, MAXIMUM_FRAME_WIDTH / video.videoWidth);
  const canvas = document.createElement("canvas");
  canvas.width = Math.round(video.videoWidth * scale);
  canvas.height = Math.round(video.videoHeight * scale);
  const context = canvas.getContext("2d", { willReadFrequently: true });
  if (!context) return null;
  context.drawImage(video, 0, 0, canvas.width, canvas.height);
  const pixels = context.getImageData(0, 0, canvas.width, canvas.height);
  lastFrameCanvas = canvas;
  return { width: pixels.width, height: pixels.height, data: pixels.data };
}

/** The canvas of the frame last analysed (for encoding the kept slide). */
let lastFrameCanvas: HTMLCanvasElement | null = null;

/** Encodes the kept slide, cropped, as JPEG. */
async function encodeSlide(
  image: RgbaImage,
  box: { left: number; top: number; right: number; bottom: number } | null,
): Promise<Uint8Array | null> {
  const source = lastFrameCanvas;
  if (!source || source.width !== image.width) return null;
  const crop = box ?? { left: 0, top: 0, right: image.width, bottom: image.height };
  const canvas = document.createElement("canvas");
  canvas.width = crop.right - crop.left;
  canvas.height = crop.bottom - crop.top;
  canvas.getContext("2d")?.drawImage(source, crop.left, crop.top, canvas.width, canvas.height, 0, 0, canvas.width, canvas.height);
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.88));
  return blob ? new Uint8Array(await blob.arrayBuffer()) : null;
}

/** One check of the page while recording. */
async function whileRecording(): Promise<void> {
  if (!recorder) return;
  recorder.observePage({ captions: settings.captureCaptions });
  const elapsed = Date.now() - recordingStartedAt;
  if (settings.captureCaptions && !captionsAttempted && elapsed >= CAPTIONS_DELAY_MS) {
    captionsAttempted = true;
    void turnOnCaptions(document);
  }
  if (
    settings.startTeamsTranscription &&
    !transcriptionAttempted &&
    elapsed >= CAPTIONS_DELAY_MS * 2 &&
    !isTeamsTranscriptRunning(document)
  ) {
    transcriptionAttempted = true;
    void startTeamsTranscription(document);
  }
  if (settings.captureSlides) {
    const video = findShareVideo(document);
    if (!video) {
      if (shareShown) recorder.resetSlides();
      shareShown = false;
    } else {
      shareShown = true;
      const frame = grabFrame(video);
      if (frame) await recorder.offerShareFrame(frame);
    }
  }
}

/** Waits for the page hook to deliver a transcript file. */
function transcriptArrival(meeting: RecordedMeeting): (timeoutMs: number) => Promise<boolean> {
  return (timeoutMs) =>
    new Promise((resolve) => {
      const timer = setTimeout(() => {
        recapFetch = null;
        resolve(false);
      }, timeoutMs);
      recapFetch = {
        meeting,
        arrived: () => {
          clearTimeout(timer);
          recapFetch = null;
          resolve(true);
        },
      };
    });
}

/** Looks for recap cards of recorded meetings and fetches their transcripts. */
async function checkRecaps(): Promise<void> {
  if (!settings.fetchTeamsTranscripts || recapFetch || Date.now() - lastRecapCheck < RECAP_CHECK_MS) return;
  lastRecapCheck = Date.now();
  const cards = findRecapCards(document).filter((card) => card.hasTranscript);
  if (cards.length === 0) return;
  const pending = pendingTranscriptMeetings(await loadMeetings());
  for (const card of cards) {
    const meeting = matchRecapToMeeting(card, pending);
    if (!meeting || recapAttempted.has(meeting.meetingId)) continue;
    recapAttempted.add(meeting.meetingId);
    await fetchRecapTranscript(card.viewRecapButton, document, transcriptArrival(meeting));
    return;
  }
}

/** Sends a transcript file the page hook copied. */
async function onTranscriptFile(text: string): Promise<void> {
  if (!text.trimStart().startsWith("WEBVTT")) return;
  let meeting = recapFetch?.meeting ?? null;
  if (!meeting) {
    // The owner opened or downloaded the transcript themselves: use the recap card on screen.
    const cards = findRecapCards(document).filter((card) => card.hasTranscript);
    const pending = pendingTranscriptMeetings(await loadMeetings());
    meeting = cards.length === 1 && cards[0] ? matchRecapToMeeting(cards[0], pending) : null;
  }
  if (!meeting) return;
  toHost({
    type: "document",
    kind: "teams_transcript",
    mediaType: "text/vtt",
    text,
    timestampMs: 0,
    meetingId: meeting.meetingId,
  });
  await saveMeeting({ ...meeting, transcriptFetched: true });
  recapFetch?.arrived();
}

/** One check of the page. */
async function tick(): Promise<void> {
  const inCall = isInCall(document);
  if (inCall) {
    if (state === "idle") {
      if (settings.alwaysRecordTeams) {
        startRecording();
      } else {
        state = "asking";
        askConsent(onConsent);
      }
    }
    if (state === "recording") await whileRecording();
    return;
  }
  if (state === "recording") stopRecording();
  if (state === "asking") hideOverlay();
  state = "idle";
  await checkRecaps();
}

window.addEventListener("message", (event: MessageEvent<HookMessage>) => {
  if (event.source !== window || event.data?.source !== HOOK_SOURCE) return;
  if (event.data.kind === "audio") recorder?.addAudio(event.data.track, new Int16Array(event.data.samples));
  if (event.data.kind === "transcript-file") void onTranscriptFile(event.data.text);
});

window.addEventListener("pagehide", () => {
  if (state === "recording") stopRecording();
});

chrome.storage.onChanged.addListener((changes, area) => {
  if (area === "local" && settingsChanged(changes)) {
    void loadSettings().then((loaded) => {
      settings = loaded;
    });
  }
});

void loadSettings().then((loaded) => {
  settings = loaded;
  let running = false;
  setInterval(() => {
    if (running) return;
    running = true;
    void tick().finally(() => {
      running = false;
    });
  }, TICK_MS);
});
