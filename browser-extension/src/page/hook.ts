/**
 * The page hook (Issue #102): runs in the Teams page's own JavaScript world at document start,
 * because only there can the extension see the microphone stream Teams opens and the remote
 * audio Teams plays, and the transcript file the page downloads.
 *
 * - Microphone: `getUserMedia` is wrapped to remember the audio tracks Teams opens. A muted
 *   microphone yields silence, which the content script does not send.
 * - Remote audio: the `<audio>` elements Teams plays a call through (one track with every remote
 *   participant mixed) are tapped with an `AudioContext`; playback is unaffected.
 * - Teams transcript: responses of the page's own transcript requests (`/transcripts/…`) are
 *   copied; the extension never makes such requests itself and never uses the page's tokens.
 *
 * Audio is tapped only between the content script's `start-audio` and `stop-audio`, converted to
 * 16 kHz PCM, and posted to the content script about once a second. Nothing is logged.
 */
import { resampleTo16k } from "../lib/pcm";
import { AudioTrack, CONTENT_SOURCE, HOOK_SOURCE, HookCommand, HookMessage } from "../lib/messages";

/** Samples per audio callback (about 85 ms at 48 kHz). */
const PROCESSOR_FRAMES = 4096;
/** How often the tapped elements and tracks are re-checked. */
const RESCAN_MS = 1_000;

/** Microphone tracks Teams opened, newest last. */
const microphoneTracks: MediaStreamTrack[] = [];

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

/** Finds the current microphone and remote streams and taps them. */
function rescan(): void {
  const microphone = [...microphoneTracks].reverse().find((track) => track.readyState === "live");
  if (microphone) {
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
        stream.getAudioTracks().some((track) => track.readyState === "live" && !microphoneTracks.includes(track)),
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

window.addEventListener("message", (event: MessageEvent<HookCommand>) => {
  if (event.source !== window || event.data?.source !== CONTENT_SOURCE) return;
  if (event.data.command === "start-audio") startAudio();
  if (event.data.command === "stop-audio") stopAudio();
});

hookGetUserMedia();
hookFetch();
