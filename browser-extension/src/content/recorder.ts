/**
 * One recording of a Teams meeting in the content script (Issue #102): it announces the meeting
 * to Note Taker, cuts the page hook's audio into chunks, follows captions and the transcript
 * panel, keeps screen-share slides, and at the end sends the captions and live transcript as
 * documents followed by the meeting end.
 *
 * All browser I/O is injected (`RecorderPorts`), so the class runs in tests. Times sent to the
 * core are milliseconds from the recording start. Nothing it handles is logged.
 */
import { CaptionCollector, TranscriptPanelCollector } from "../lib/captions";
import { HostMessage, HostParticipant, AudioTrack } from "../lib/messages";
import { PcmChunker, base64FromBytes, bytesFromSamples, isSilent, TARGET_SAMPLE_RATE } from "../lib/pcm";
import { SlideChangeDetector } from "../lib/slides";
import { readCaptions, readTranscriptPanel, isTeamsTranscriptRunning } from "../lib/teamsDom";

/** A gap this large between the audio received and the clock restarts a track's timing. */
const TIMING_GAP_MS = 3_000;
/** The owner's microphone track, as Teams participant, when the owner's name is unknown. */
const OWNER_PARTICIPANT_ID = "browser-owner";

/**
 * The participant ID of the owner's microphone track.
 *
 * The app maps a participant ID to a person once and keeps that person's name, so the ID carries
 * the owner's name: another owner of the same browser, or a name read correctly after a Teams
 * change, gets a person of their own instead of inheriting an earlier one.
 *
 * Inputs: The owner's name from the Teams page ("Me" when unknown).
 * Outputs: `browser-owner:<name>` in lower case with single spaces, or `browser-owner`.
 * Side effects: None.
 */
export function ownerParticipantId(ownerName: string): string {
  const normalized = ownerName.trim().replace(/\s+/g, " ").toLowerCase();
  return normalized === "" || normalized === "me" ? OWNER_PARTICIPANT_ID : `${OWNER_PARTICIPANT_ID}:${normalized}`;
}

/** What the recorder needs from the browser. */
export interface RecorderPorts {
  /** Sends a message to Note Taker. */
  send(message: HostMessage): void;
  /** The current time (epoch ms). */
  now(): number;
  /** The page to read captions and the transcript panel from. */
  root: ParentNode;
}

/** The meeting being recorded. */
export interface RecorderMeeting {
  meetingId: string;
  title: string;
  ownerName: string;
}

/** One track's chunking state. */
interface TrackTiming {
  chunker: PcmChunker;
  /** Meeting time of the chunker's first sample. */
  startMs: number;
  /** Samples received so far. */
  samples: number;
}

/** Follows one meeting from start to end. */
export class MeetingRecorder {
  readonly startMs: number;
  private readonly captions = new CaptionCollector();
  private readonly transcriptPanel = new TranscriptPanelCollector();
  private readonly slides = new SlideChangeDetector();
  private readonly tracks = new Map<AudioTrack, TrackTiming>();
  private teamsTranscriptSeen = false;
  private stopped = false;
  private sentChunks = 0;
  private sentSlides = 0;
  /** When the slide awaiting `sendSlide` settled, in meeting time. */
  private pendingSlideMs: number | null = null;

  /** Starts the recording and announces the meeting. Side effects: Sends `meeting`. */
  constructor(
    private readonly meeting: RecorderMeeting,
    private readonly ports: RecorderPorts,
  ) {
    this.startMs = ports.now();
    ports.send({
      type: "meeting",
      meetingId: meeting.meetingId,
      title: meeting.title,
      startTimeMs: this.startMs,
      participants: [this.owner()],
    });
  }

  /** The owner as the microphone track's participant. */
  private owner(): HostParticipant {
    return { id: ownerParticipantId(this.meeting.ownerName), name: this.meeting.ownerName };
  }

  /** Milliseconds since the recording started. */
  private elapsedMs(): number {
    return Math.max(0, this.ports.now() - this.startMs);
  }

  /**
   * Takes about a second of 16 kHz audio from the page hook.
   *
   * Inputs: The track and its samples. Side effects: May send audio chunks (silent ones are
   * skipped); restarts the track's timing after a gap.
   */
  addAudio(track: AudioTrack, samples: Int16Array): void {
    if (this.stopped) return;
    const nowMs = this.elapsedMs();
    let timing = this.tracks.get(track);
    const receivedMs = timing ? timing.startMs + (timing.samples / TARGET_SAMPLE_RATE) * 1000 : 0;
    const arrivedStartMs = Math.max(0, nowMs - (samples.length / TARGET_SAMPLE_RATE) * 1000);
    if (timing && Math.abs(arrivedStartMs - receivedMs) > TIMING_GAP_MS) {
      timing.chunker.flush();
      timing = undefined;
    }
    if (!timing) {
      const startMs = Math.round(arrivedStartMs);
      timing = {
        startMs,
        samples: 0,
        chunker: new PcmChunker(startMs, (chunk) => {
          if (isSilent(chunk.samples)) return;
          this.sentChunks += 1;
          this.ports.send({
            type: "audio",
            track,
            ...(track === "microphone" ? { participant: this.owner() } : {}),
            timestampMs: chunk.timestampMs,
            pcm: base64FromBytes(bytesFromSamples(chunk.samples)),
          });
        }),
      };
      this.tracks.set(track, timing);
    }
    timing.samples += samples.length;
    timing.chunker.push(samples);
  }

  /**
   * Reads captions and the transcript panel (call about once a second).
   * Side effects: Updates the collectors.
   */
  observePage(options: { captions: boolean }): void {
    if (this.stopped) return;
    const nowMs = this.elapsedMs();
    if (options.captions) this.captions.observe(readCaptions(this.ports.root), nowMs);
    if (isTeamsTranscriptRunning(this.ports.root)) this.teamsTranscriptSeen = true;
    this.transcriptPanel.observe(readTranscriptPanel(this.ports.root), nowMs);
  }

  /**
   * Offers the signature of a screen-share frame (`signatureOf`, computed by the page hook; call
   * about once a second while a share is shown).
   *
   * Inputs: The signature. Outputs: `true` when the share changed and settled, so that frame
   * should be sent with `sendSlide`. Side effects: Remembers when it settled.
   */
  offerShareSignature(signature: ArrayLike<number>): boolean {
    if (this.stopped || !this.slides.offer(Float32Array.from(signature))) return false;
    this.pendingSlideMs = this.elapsedMs();
    return true;
  }

  /**
   * Sends the slide asked for by `offerShareSignature`, stamped with when it settled.
   *
   * Inputs: The cropped slide as base64 JPEG. Side effects: Sends it once; otherwise nothing.
   */
  sendSlide(imageBase64: string): void {
    const timestampMs = this.pendingSlideMs;
    this.pendingSlideMs = null;
    if (this.stopped || timestampMs === null || imageBase64 === "") return;
    this.sentSlides += 1;
    this.ports.send({ type: "slide", timestampMs, image: imageBase64 });
  }

  /** A new share started: the next settled frame is kept even if it looks like the last. */
  resetSlides(): void {
    this.slides.reset();
  }

  /** Whether Teams transcription was seen during the recording. */
  sawTeamsTranscript(): boolean {
    return this.teamsTranscriptSeen;
  }

  /** Counts for the popup. */
  counts(): { audioChunks: number; slides: number; captions: number } {
    return { audioChunks: this.sentChunks, slides: this.sentSlides, captions: this.captions.cues().length };
  }

  /**
   * Ends the recording: flushes audio, sends the captions and live transcript, then the end.
   * Side effects: Sends messages; later calls do nothing.
   */
  stop(): void {
    if (this.stopped) return;
    for (const timing of this.tracks.values()) timing.chunker.flush();
    this.stopped = true;
    if (!this.captions.isEmpty()) {
      this.ports.send({
        type: "document",
        kind: "teams_captions",
        mediaType: "text/vtt",
        text: this.captions.toVtt(),
        timestampMs: 0,
      });
    }
    if (!this.transcriptPanel.isEmpty()) {
      this.ports.send({
        type: "document",
        kind: "teams_live_transcript",
        mediaType: "text/vtt",
        text: this.transcriptPanel.toVtt(),
        timestampMs: 0,
      });
    }
    this.ports.send({ type: "ended", meetingId: this.meeting.meetingId });
  }
}
