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
import { loadMeetings, loadSettings, saveMeeting, saveSettings, settingsChanged } from "../lib/storage";
import {
  findRecapCards,
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
    { send: toHost, now: Date.now, root: document },
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
  lastProblem = null;
  lastProgress = "";
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
    // The page hook draws the frame and answers with "share-frame" (see the message listener).
    toHook("analyse-share");
  }
  reportProgress();
}

/** The last step that failed while recording, as "<step>: <error name>". */
let lastProblem: string | null = null;
/** The progress line last sent to the background, to send only changes. */
let lastProgress = "";

/** Tells the background what was sent so far, when it changed. */
function reportProgress(): void {
  if (!recorder || !record) return;
  const progress = { title: record.title, ...recorder.counts(), problem: lastProblem };
  const serialized = JSON.stringify(progress);
  if (serialized === lastProgress) return;
  lastProgress = serialized;
  toBackground({ kind: "progress", progress });
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
  if (event.data.kind === "share-frame") onShareFrame(event.data.present, event.data.signature);
  if (event.data.kind === "slide-image") {
    if (event.data.image === "") lastProblem = "slides: the shared screen could not be read";
    recorder?.sendSlide(event.data.image);
  }
});

/**
 * Handles the hook's analysis of the screen share.
 * Inputs: Whether a share is shown and its frame's signature (empty while it has no picture).
 * Side effects: Restarts slide detection when a share ends; asks the hook to encode a settled slide.
 */
function onShareFrame(present: boolean, signature: number[]): void {
  if (!recorder || state !== "recording") return;
  if (!present) {
    if (shareShown) recorder.resetSlides();
    shareShown = false;
    return;
  }
  shareShown = true;
  if (signature.length > 0 && recorder.offerShareSignature(signature)) toHook("encode-slide");
}

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
