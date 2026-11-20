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
 * calls, when Teams shows a recap with a transcript for a meeting it recorded (the recap a meeting
 * chat opens on, or a recap card), it opens the Transcript tab and has the recap's SharePoint
 * frame download the WebVTT (see `src/recap/`), then sends it to the same Koalino meeting.
 *
 * Everything goes to the background over a port, and from there to the native host. Nothing of
 * the meeting is logged.
 */
import { msg, type MessageKey } from "../lib/i18n";
import {
  RecapFetchOutcome,
  fetchRecapTranscript,
  setTeamsSpokenLanguage,
  startTeamsTranscription,
  turnOnCaptions,
} from "./automation";
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
import {
  RecapAttempts,
  RecordedMeeting,
  heartbeatDue,
  matchRecapToMeeting,
  newMeetingId,
  pendingTranscriptMeetings,
  recapRetryDue,
  leftMeetingToClose,
  resumableMeeting,
} from "../lib/meetings";
import { ExtensionSettings, DEFAULT_SETTINGS } from "../lib/settings";
import { loadMeetings, loadSettings, saveMeeting, saveSettings, settingsChanged } from "../lib/storage";
import {
  findRecapCards,
  findRecapView,
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
/** Whether Teams' spoken language was set to Koalino's in this recording (#141). */
let languageAttempted = false;
let shareShown = false;
let lastRecapCheck = 0;
let recapFetch: { meeting: RecordedMeeting; arrived: () => void } | null = null;
/** The tries at each meeting's recap transcript (by meeting ID) since this page loaded. */
const recapAttempts = new Map<string, RecapAttempts>();
/** The last Teams transcript fetch status sent to the popup. */
let recapStatus = "";
/** Meetings whose Teams transcript this page already sent (by meeting ID). */
const transcriptsSent = new Set<string>();
let overlayStatus = "";

/** The last step that failed while recording, as "<step>: <error name>". */
let lastProblem: string | null = null;
/** The progress line last sent to the background, to send only changes. */
let lastProgress = "";

let port = connectPort();

/** Opens the port to the background (again, after the background restarted). */
function connectPort(): chrome.runtime.Port {
  const opened = chrome.runtime.connect({ name: PORT_NAME });
  opened.onMessage.addListener((message: BackgroundMessage) => {
    if (message.kind === "connection") {
      connection = message.state;
      refreshOverlay();
    }
    if (message.kind === "recap-transcript") void onTranscriptFile(message.text);
  });
  opened.onDisconnect.addListener(() => {
    port = connectPort();
  });
  // Firefox stops an idle background and loses what it knew: tell it again.
  if (state === "recording" && record) {
    opened.postMessage({
      kind: "recording",
      recording: true,
      title: record.title,
      meetingId: record.meetingId,
    } satisfies TabMessage);
    lastProgress = "";
  }
  if (recapStatus) opened.postMessage({ kind: "recap-status", status: recapStatus } satisfies TabMessage);
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

/** Sends a message on to Koalino. */
function toHost(message: HostMessage): void {
  toBackground({ kind: "host", message });
}

/** Sends a command to the page hook. */
function toHook(command: HookCommand["command"]): void {
  window.postMessage({ source: CONTENT_SOURCE, command } satisfies HookCommand, window.location.origin);
}

/** The recording overlay's status line. */
function statusLine(): string {
  if (connection.host === "missing") return msg("statusHelperMissing");
  if (connection.bridge === "unavailable") return msg("statusAppNotRunning");
  if (connection.bridge === "connected") return msg("statusSending");
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

/** Remembers, in this tab only, the meeting it records; a reload keeps it (#102). */
const TAB_MEETING_KEY = "koalino-recording-meeting";

/** The meeting this tab was recording before it reloaded, or `null`. */
function tabMeetingId(): string | null {
  try {
    return window.sessionStorage.getItem(TAB_MEETING_KEY);
  } catch {
    return null;
  }
}

/** Remembers (or, with `null`, forgets) the meeting this tab records. */
function rememberTabMeeting(meetingId: string | null): void {
  try {
    if (meetingId === null) window.sessionStorage.removeItem(TAB_MEETING_KEY);
    else window.sessionStorage.setItem(TAB_MEETING_KEY, meetingId);
  } catch {
    // Storage blocked: a reload then starts a new meeting, as before #102.
  }
}

/**
 * Starts recording the current call, or continues `resumed`, the meeting this tab recorded
 * before a reload (#102): same meeting ID and start, so Koalino keeps one meeting.
 */
function startRecording(resumed: RecordedMeeting | null = null): void {
  const nowMs = Date.now();
  const startMs = resumed?.startMs ?? nowMs;
  const meetingId = resumed?.meetingId ?? newMeetingId(startMs);
  const title = resumed?.title || meetingTitleFromDocumentTitle(document.title);
  recorder = new MeetingRecorder(
    { meetingId, title, ownerName: ownerName(document), startMs },
    { send: toHost, now: Date.now, root: document },
  );
  record = resumed
    ? { ...resumed, lastSeenMs: nowMs }
    : {
        meetingId,
        title,
        startMs,
        endMs: null,
        teamsTranscriptSeen: false,
        transcriptFetched: false,
      };
  void saveMeeting(record);
  rememberTabMeeting(meetingId);
  state = "recording";
  recordingStartedAt = nowMs;
  captionsAttempted = false;
  transcriptionAttempted = false;
  languageAttempted = false;
  lastProblem = null;
  lastProgress = "";
  shareShown = false;
  overlayStatus = "\u0000";
  toHook("start-audio");
  toBackground({ kind: "recording", recording: true, title, meetingId });
  refreshOverlay();
}

/** Ends the recording (the call ended or the owner pressed Stop). */
function stopRecording(): void {
  if (!recorder || !record) return;
  toHook("stop-audio");
  recorder.stop();
  record = { ...record, endMs: Date.now(), teamsTranscriptSeen: recorder.sawTeamsTranscript() };
  void saveMeeting(record);
  rememberTabMeeting(null);
  toBackground({ kind: "recording", recording: false, title: record.title, meetingId: record.meetingId });
  recorder = null;
  hideOverlay();
}

/**
 * The call ended in the page, perhaps dropped: send what was collected, but keep the meeting
 * open, so rejoining the same call in this tab within two minutes continues it (as a reload
 * does, #102). Otherwise the background ends it, and this tab marks it ended.
 * Side effects: Sends audio and captions; tells the background; hides the overlay.
 */
function leaveCall(): void {
  if (!recorder || !record) return;
  const { title, meetingId } = record;
  interruptRecording();
  toBackground({ kind: "recording", recording: false, title, meetingId, interrupted: true });
  hideOverlay();
}

/**
 * Marks ended the meeting this tab left, once it can no longer be rejoined.
 * Side effects: Updates the stored record; forgets the tab's meeting.
 */
async function closeLeftMeeting(): Promise<void> {
  const closed = leftMeetingToClose(await loadMeetings(), tabMeetingId(), Date.now());
  if (!closed) return;
  await saveMeeting(closed);
  rememberTabMeeting(null);
}

/**
 * The page is unloading mid-call, perhaps to reload (#102): send what was collected, but leave
 * the meeting open. The reloaded page continues it; if none does, the background ends it.
 * Side effects: Sends audio and captions; notes the time in the record.
 */
function interruptRecording(): void {
  if (!recorder || !record) return;
  toHook("stop-audio");
  recorder.interrupt();
  record = { ...record, lastSeenMs: Date.now(), teamsTranscriptSeen: recorder.sawTeamsTranscript() };
  void saveMeeting(record);
  recorder = null;
}

/**
 * Notes in the record that the recording still runs, every `HEARTBEAT_MS`, so a page reloaded
 * during the call leaves an approximate end for matching its Teams transcript (#102).
 * Side effects: Writes the record to storage.
 */
function noteStillRecording(): void {
  const now = Date.now();
  if (!record || !heartbeatDue(record, now)) return;
  record = { ...record, lastSeenMs: now };
  void saveMeeting(record);
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
  // Before Teams transcription starts, so it starts in Koalino's language (#141). Only once
  // Koalino has named a fixed language; the steps run one after another.
  if (
    settings.matchTeamsSpokenLanguage &&
    !languageAttempted &&
    connection.language &&
    elapsed >= CAPTIONS_DELAY_MS
  ) {
    languageAttempted = true;
    void setTeamsSpokenLanguage(document, connection.language).then((outcome) => {
      if (outcome === "failed") lastProblem = "spoken language: not set";
    });
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

/** Asks the tab's recap frames, through the background, to download the recap transcript. */
function requestRecapDownload(): void {
  toBackground({ kind: "recap-download" });
}

/**
 * The recaps on screen with a transcript: the recap a meeting chat opens on, and recap cards in
 * the chat (each with its "View recap" button).
 */
function recapsOnScreen(): { title: string; startMs: number | null; endMs: number | null; open: HTMLElement | null }[] {
  const view = findRecapView(document);
  const cards = findRecapCards(document)
    .filter((card) => card.hasTranscript)
    .map((card) => ({ title: card.title, startMs: card.startMs, endMs: card.endMs, open: card.viewRecapButton }));
  return view ? [{ title: view.title, startMs: view.startMs, endMs: view.endMs, open: null }, ...cards] : cards;
}

/** Tells the popup what happened to the Teams transcript fetch, when it changed. */
function reportRecapStatus(status: string): void {
  if (status === recapStatus) return;
  recapStatus = status;
  toBackground({ kind: "recap-status", status });
}

/** A recap's session time as "16:10–16:21", or "" when not shown. */
function sessionTime(recap: { startMs: number | null; endMs: number | null }): string {
  const time = (ms: number) => new Date(ms).toTimeString().slice(0, 5);
  if (recap.startMs === null) return "";
  return recap.endMs === null ? time(recap.startMs) : `${time(recap.startMs)}–${time(recap.endMs)}`;
}

/** What a failed fetch step means, for the popup. */
const RECAP_FAILURES: Record<Exclude<RecapFetchOutcome, "sent">, MessageKey> = {
  "no-transcript-tab": "recapNoTranscriptTab",
  "nothing-arrived": "recapNothingArrived",
};

/** Looks for recaps of recorded meetings and fetches their transcripts. */
async function checkRecaps(): Promise<void> {
  if (!settings.fetchTeamsTranscripts || recapFetch || Date.now() - lastRecapCheck < RECAP_CHECK_MS) return;
  lastRecapCheck = Date.now();
  try {
    const recaps = recapsOnScreen();
    if (recaps.length === 0) return;
    const meetings = await loadMeetings();
    const pending = pendingTranscriptMeetings(meetings);
    for (const recap of recaps) {
      const meeting = matchRecapToMeeting(recap, pending);
      if (!meeting) {
        const sent = matchRecapToMeeting(
          recap,
          meetings.filter((recorded) => recorded.transcriptFetched),
        );
        reportRecapStatus(
          sent
            ? msg("transcriptAlreadySent", { title: sent.title || msg("teamsMeeting") })
            : msg("transcriptNoMatch", { time: sessionTime(recap) || msg("noTimeShown") }),
        );
        continue;
      }
      const attempts = recapAttempts.get(meeting.meetingId);
      if (!recapRetryDue(attempts, Date.now())) continue;
      recapAttempts.set(meeting.meetingId, { count: (attempts?.count ?? 0) + 1, lastMs: Date.now() });
      const title = meeting.title || msg("teamsMeeting");
      reportRecapStatus(msg("fetchingTranscript", { title }));
      const outcome = await fetchRecapTranscript(document, recap.open, requestRecapDownload, transcriptArrival(meeting));
      const triesLeft = recapRetryDue({ count: (attempts?.count ?? 0) + 1, lastMs: 0 }, Date.now());
      reportRecapStatus(
        outcome === "sent"
          ? msg("transcriptSent", { title })
          : msg(triesLeft ? "transcriptFailedRetrying" : "transcriptFailed", {
              title,
              reason: msg(RECAP_FAILURES[outcome]),
            }),
      );
      return;
    }
  } catch (error) {
    reportRecapStatus(msg("transcriptError", { error: error instanceof Error ? error.name : "error" }));
  }
}

/** Sends a transcript file the page hook copied. */
async function onTranscriptFile(text: string): Promise<void> {
  if (!text.trimStart().startsWith("WEBVTT")) return;
  let meeting = recapFetch?.meeting ?? null;
  if (!meeting) {
    // The owner downloaded the transcript themselves: use the one recap on screen.
    const recaps = recapsOnScreen();
    const pending = pendingTranscriptMeetings(await loadMeetings());
    meeting = recaps.length === 1 && recaps[0] ? matchRecapToMeeting(recaps[0], pending) : null;
  }
  // The recap player can fetch the file twice for one download: send each meeting's once. The
  // check and the mark happen together, before any wait, so a second copy cannot slip through.
  if (!meeting || transcriptsSent.has(meeting.meetingId)) return;
  transcriptsSent.add(meeting.meetingId);
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
      // The owner already chose to record this call before the page reloaded: continue it.
      const resumed = resumableMeeting(
        await loadMeetings(),
        tabMeetingId(),
        Date.now(),
        meetingTitleFromDocumentTitle(document.title),
      );
      if (resumed) {
        startRecording(resumed);
      } else if (settings.alwaysRecordTeams) {
        startRecording();
      } else {
        state = "asking";
        askConsent(onConsent);
      }
    }
    if (state === "recording") {
      noteStillRecording();
      await whileRecording();
    }
    return;
  }
  if (state === "recording") leaveCall();
  if (state === "asking") hideOverlay();
  state = "idle";
  await closeLeftMeeting();
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
  if (state === "recording") interruptRecording();
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
