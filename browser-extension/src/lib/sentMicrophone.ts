/**
 * Finds the owner's microphone in Teams' call connections (#157).
 *
 * The page hook first learned the microphone by wrapping `getUserMedia`, which misses a
 * microphone Teams opened before the hook ran: on the pre-join screen before the extension
 * loaded into the tab, or when the extension is installed or updated during a call. A call
 * connection (`RTCPeerConnection`) always knows the track it sends. Methods patched on its
 * prototype also run for connections that already exist, and Teams calls `getStats` on its
 * connection about once a second, so the hook meets every call connection within a second.
 *
 * Once a connection with an audio sender is known, that sender decides: a live track is the
 * microphone, and an empty or ended one means Teams muted it. A muted call is never filled in with
 * an earlier `getUserMedia` track, which might still be hearing the room. Nothing is logged.
 *
 * Firefox injects the hook again into open Teams tabs when the extension is installed, updated or
 * reloaded, and the earlier copy stays in the page. `claimHookGeneration` lets only the newest
 * copy act, so audio is never tapped and sent twice.
 */

/** The page-wide key under which the newest hook's generation is kept. */
const HOOK_GENERATION_KEY = Symbol.for("koalino.pageHookGeneration");

/**
 * Claims the next hook generation in a page.
 *
 * Inputs: The page's global object (`window`), or any object standing in for it.
 * Outputs: A function telling whether this claim is still the newest in that page.
 * Side effects: Stores the new generation on `page` under a registered symbol.
 */
export function claimHookGeneration(page: object): () => boolean {
  const holder = page as Record<symbol, unknown>;
  const previous = holder[HOOK_GENERATION_KEY];
  const generation = (typeof previous === "number" ? previous : 0) + 1;
  holder[HOOK_GENERATION_KEY] = generation;
  return () => holder[HOOK_GENERATION_KEY] === generation;
}

/** The parts of a `MediaStreamTrack` read here. */
export interface TrackLike {
  kind: string;
  readyState: string;
}

/** The parts of an `RTCRtpTransceiver` read here. */
export interface TransceiverLike {
  sender: { track: TrackLike | null };
  receiver: { track: TrackLike | null };
  /** The negotiated direction ("sendrecv", "sendonly", …), or null before negotiation. */
  currentDirection?: string | null;
  /** The wanted direction, used while `currentDirection` is still null. */
  direction?: string;
}

/** The parts of an `RTCPeerConnection` read here. */
export interface ConnectionLike {
  connectionState?: string;
  getTransceivers(): TransceiverLike[];
}

/**
 * What the call connections say about the microphone.
 *
 * - `unknown`: no open connection sends audio (no call yet, or none seen): use other sources.
 * - `track`: the live track Teams sends.
 * - `muted`: Teams sends audio but with no live track: tap no microphone at all.
 */
export type SentMicrophone<Track extends TrackLike = TrackLike> =
  | { kind: "unknown" }
  | { kind: "track"; track: Track }
  | { kind: "muted" };

/** Whether a transceiver direction sends media. */
function sends(direction: string | null | undefined): boolean {
  return direction === "sendrecv" || direction === "sendonly";
}

/**
 * Finds the microphone track Teams sends.
 *
 * Inputs: The call connections seen so far, in any state.
 * Outputs: The first live audio track sent by an open connection; else `muted` when an open
 * connection has an audio sender without a live track; else `unknown`.
 * Side effects: None (a connection whose transceivers throw is skipped).
 */
export function sentMicrophone<Track extends TrackLike>(
  connections: Iterable<ConnectionLike>,
): SentMicrophone<Track> {
  let sawAudioSender = false;
  for (const connection of connections) {
    if (connection.connectionState === "closed") continue;
    let transceivers: TransceiverLike[];
    try {
      transceivers = connection.getTransceivers();
    } catch {
      continue;
    }
    for (const transceiver of transceivers) {
      const sent = transceiver.sender.track;
      // A sender without a track has no kind; the receiver's track tells what the line carries.
      const isAudio = sent ? sent.kind === "audio" : transceiver.receiver.track?.kind === "audio";
      if (!isAudio || !sends(transceiver.currentDirection ?? transceiver.direction)) continue;
      if (sent && sent.readyState === "live") return { kind: "track", track: sent as Track };
      sawAudioSender = true;
    }
  }
  return sawAudioSender ? { kind: "muted" } : { kind: "unknown" };
}

/**
 * The connection methods a call page calls: Teams calls `getStats` about once a second. Other
 * services (#174) may receive only, so the negotiation steps every connection takes are watched
 * too.
 */
const OBSERVED_METHODS = [
  "getStats",
  "addTrack",
  "addTransceiver",
  "setLocalDescription",
  "setRemoteDescription",
] as const;

/**
 * Patches a connection prototype so every connection is reported when Teams next uses it.
 *
 * Inputs: `RTCPeerConnection.prototype` (or undefined where there is none); `seen`, called with
 * the connection before each patched call.
 * Outputs: None.
 * Side effects: Replaces `OBSERVED_METHODS` on the prototype with wrappers that call `seen` and
 * then the original with the same arguments and result. A throwing
 * `seen` is swallowed, so the page's own call always runs.
 */
export function observeConnections<Connection extends object>(
  prototype: Connection | undefined,
  seen: (connection: Connection) => void,
): void {
  if (!prototype) return;
  const methods = prototype as unknown as Record<string, unknown>;
  for (const name of OBSERVED_METHODS) {
    const original = methods[name];
    if (typeof original !== "function") continue;
    methods[name] = function (this: Connection, ...args: unknown[]): unknown {
      try {
        seen(this);
      } catch {
        // Never disturb the call.
      }
      return (original as (...callArgs: unknown[]) => unknown).apply(this, args);
    };
  }
}
