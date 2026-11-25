/**
 * Tells, on any website, that a call is going on (#174).
 *
 * The page hook sees the page's call connections (`RTCPeerConnection`, `observeConnections`) and
 * the microphone and camera tracks it opened (`getUserMedia`). A call is an open connection that
 * carries audio and receives someone's audio; it goes on while that connection stays open. The
 * microphone or camera alone is no call: dictation, a voice recorder or a camera test use them
 * too, and a pre-join screen sends the camera and microphone before the call starts; so a call
 * starts only once someone's audio comes in. A connection without audio (a data channel, a file
 * transfer) is no call either.
 *
 * `CallDetector` smooths the answer: a call starts once it has looked like one for a few seconds
 * and ends once it has not for a few more, so an ICE restart or a muted moment does not end it.
 * Structural types keep this testable without a browser. Nothing is logged.
 */

/** The parts of a `MediaStreamTrack` read here. */
export interface MediaTrackLike {
  kind: string;
  readyState: string;
  /** Whether a received track gets no media right now. */
  muted?: boolean;
}

/** The parts of an `RTCRtpTransceiver` read here. */
export interface TransceiverLike {
  sender: { track: MediaTrackLike | null };
  receiver: { track: MediaTrackLike | null };
  currentDirection?: string | null;
  direction?: string;
}

/** The parts of an `RTCPeerConnection` read here. */
export interface ConnectionLike {
  connectionState?: string;
  iceConnectionState?: string;
  getTransceivers(): TransceiverLike[];
}

/** What the page is doing with calls and media right now. */
export interface CallActivity {
  /** An open connection carries audio. */
  connected: boolean;
  /** The page holds a live microphone track (opened, or sent on a connection). */
  microphone: boolean;
  /** The page holds a live camera track (a shared screen does not count). */
  camera: boolean;
  /** A connection receives someone's audio right now. */
  remoteAudio: boolean;
}

/** Whether a connection is open: connected, or reconnecting after it was. */
function isOpen(connection: ConnectionLike): boolean {
  const state = connection.connectionState;
  if (state !== undefined) return state === "connected" || state === "disconnected";
  const ice = connection.iceConnectionState;
  return ice === "connected" || ice === "completed" || ice === "disconnected";
}

/** Whether a connection may still carry media (not closed or failed). */
function isUsable(connection: ConnectionLike): boolean {
  const state = connection.connectionState ?? connection.iceConnectionState;
  return state !== "closed" && state !== "failed";
}

/** A connection's transceivers, or none when it throws (closed meanwhile). */
function transceiversOf(connection: ConnectionLike): TransceiverLike[] {
  try {
    return connection.getTransceivers();
  } catch {
    return [];
  }
}

/** Whether a track is live. */
function isLive(track: MediaTrackLike | null | undefined): track is MediaTrackLike {
  return track !== null && track !== undefined && track.readyState === "live";
}

/**
 * What the page is doing with calls and media.
 *
 * Inputs: The call connections seen so far (any state); the microphone and camera tracks the
 * page opened with `getUserMedia`; the tracks it shares with `getDisplayMedia`, which are not
 * the camera even when a connection sends them.
 * Outputs: The activity. Side effects: None.
 */
export function callActivity(
  connections: Iterable<ConnectionLike>,
  localTracks: readonly MediaTrackLike[],
  displayTracks: readonly MediaTrackLike[] = [],
): CallActivity {
  const activity: CallActivity = {
    connected: false,
    microphone: localTracks.some((track) => track.kind === "audio" && isLive(track)),
    camera: localTracks.some((track) => track.kind === "video" && isLive(track) && !displayTracks.includes(track)),
    remoteAudio: false,
  };
  for (const connection of connections) {
    if (!isOpen(connection)) continue;
    for (const transceiver of transceiversOf(connection)) {
      const sent = transceiver.sender.track;
      const received = transceiver.receiver.track;
      const carriesAudio = (sent ?? received)?.kind === "audio";
      if (carriesAudio) activity.connected = true;
      if (isLive(sent) && sent.kind === "audio") activity.microphone = true;
      if (isLive(sent) && sent.kind === "video" && !displayTracks.includes(sent)) activity.camera = true;
      if (isLive(received) && received.kind === "audio" && received.muted !== true) activity.remoteAudio = true;
    }
  }
  return activity;
}

/**
 * Whether the activity is a call.
 *
 * A call starts once an open connection receives someone's audio. Sending the microphone and the
 * camera is not enough: Google Meet's pre-join screen already sends both on a connected
 * connection before anyone else is heard (#174 live test). Once started, the call goes on while
 * the connection stays open, so a silent stretch (nobody speaks, everyone muted) does not end it.
 *
 * Inputs: The activity, and whether a call is already on.
 * Outputs: `true` for a call. Side effects: None.
 */
export function looksLikeCall(activity: CallActivity, alreadyInCall = false): boolean {
  if (alreadyInCall) return activity.connected;
  return activity.connected && activity.remoteAudio;
}

/**
 * Every live audio track the page receives on its connections, for recording the other side of
 * a call that plays them separately (Google Meet plays the loudest speakers as separate tracks).
 *
 * Inputs: The call connections seen so far. Outputs: The tracks, each once, in connection order;
 * closed or failed connections and ended tracks are left out. Muted tracks are kept: they are
 * silent now but may carry a speaker later. Side effects: None.
 */
export function remoteAudioTracks<Track extends MediaTrackLike>(connections: Iterable<ConnectionLike>): Track[] {
  const tracks: Track[] = [];
  for (const connection of connections) {
    if (!isUsable(connection)) continue;
    for (const transceiver of transceiversOf(connection)) {
      const received = transceiver.receiver.track;
      if (isLive(received) && received.kind === "audio" && !tracks.includes(received as Track)) {
        tracks.push(received as Track);
      }
    }
  }
  return tracks;
}

/** A change in the call state. */
export type CallChange = "started" | "ended";

/** Turns moment-to-moment answers into a call that starts and ends. */
export class CallDetector {
  private callOn = false;
  /** When the current run of the opposite answer began, or null. */
  private changingSince: number | null = null;

  /**
   * Inputs: How long a call must look like one to start, and how long it must not to end (ms).
   */
  constructor(
    private readonly startMs: number,
    private readonly endGraceMs: number,
  ) {}

  /** Whether a call is on. */
  get inCall(): boolean {
    return this.callOn;
  }

  /**
   * Takes one answer.
   *
   * Inputs: Whether the page looks like a call now, and the time (ms, any steady clock).
   * Outputs: `started` or `ended` when the call state changed, else null.
   * Side effects: Updates the detector.
   */
  observe(looksLikeCallNow: boolean, nowMs: number): CallChange | null {
    if (looksLikeCallNow === this.callOn) {
      this.changingSince = null;
      return null;
    }
    this.changingSince ??= nowMs;
    const needed = this.callOn ? this.endGraceMs : this.startMs;
    if (nowMs - this.changingSince < needed) return null;
    this.callOn = looksLikeCallNow;
    this.changingSince = null;
    return this.callOn ? "started" : "ended";
  }
}
