/**
 * Specifies how the page hook tells, on any website, that a call is going on (#174): an open
 * call connection carrying audio that receives someone's audio, held for a few seconds; it goes
 * on while the connection stays open, and is over a few seconds after it closes.
 */
import { describe, expect, it } from "vitest";
import {
  CallDetector,
  ConnectionLike,
  MediaTrackLike,
  callActivity,
  looksLikeCall,
  remoteAudioTracks,
} from "../src/lib/callSignal";

/** A track of `kind`, live and receiving by default. */
function track(kind: string, readyState = "live", muted = false): MediaTrackLike {
  return { kind, readyState, muted };
}

/** A connection with one transceiver per entry. */
function connection(
  transceivers: { sent?: MediaTrackLike | null; received?: MediaTrackLike | null; direction?: string }[],
  connectionState = "connected",
): ConnectionLike {
  return {
    connectionState,
    getTransceivers: () =>
      transceivers.map(({ sent = null, received = null, direction = "sendrecv" }) => ({
        sender: { track: sent },
        receiver: { track: received },
        currentDirection: direction,
      })),
  };
}

describe("callActivity", () => {
  it("sees nothing on a page without connections or media", () => {
    expect(callActivity([], [])).toEqual({ connected: false, microphone: false, camera: false, remoteAudio: false });
  });

  it("sees a connected call that sends the microphone and receives audio", () => {
    const call = connection([{ sent: track("audio"), received: track("audio") }]);
    expect(callActivity([call], [])).toEqual({ connected: true, microphone: true, camera: false, remoteAudio: true });
  });

  it("counts a camera the page opened, but not a shared screen", () => {
    const camera = track("video");
    const screen = track("video");
    const call = connection([{ received: track("audio") }, { sent: screen, received: track("video") }]);
    expect(callActivity([call], [camera], [screen]).camera).toBe(true);
    expect(callActivity([call], [], [screen]).camera).toBe(false);
  });

  it("ignores closed connections, ended tracks and muted remote audio", () => {
    const closed = connection([{ sent: track("audio"), received: track("audio") }], "closed");
    expect(callActivity([closed], []).connected).toBe(false);
    const quiet = connection([{ sent: track("audio", "ended"), received: track("audio", "live", true) }]);
    expect(callActivity([quiet], [])).toMatchObject({ connected: true, microphone: false, remoteAudio: false });
  });

  it("does not take a connection that carries no audio (a data channel or a file transfer) as a call", () => {
    const data = connection([{ received: track("video") }]);
    expect(callActivity([data], [track("audio")]).connected).toBe(false);
  });

  it("falls back on the ICE state where connectionState is missing", () => {
    const older: ConnectionLike = {
      iceConnectionState: "completed",
      getTransceivers: () => [{ sender: { track: null }, receiver: { track: track("audio") } }],
    };
    expect(callActivity([older], []).connected).toBe(true);
  });

  it("skips a connection whose transceivers throw", () => {
    const broken: ConnectionLike = {
      connectionState: "connected",
      getTransceivers: () => {
        throw new Error("closed");
      },
    };
    expect(callActivity([broken], []).connected).toBe(false);
  });
});

describe("looksLikeCall", () => {
  it("starts only once someone's audio comes in on an open connection", () => {
    expect(looksLikeCall({ connected: true, microphone: false, camera: false, remoteAudio: true })).toBe(true);
    // Google Meet's pre-join screen already sends the microphone and camera on a connected
    // connection, but receives nobody (seen in the #174 live test).
    expect(looksLikeCall({ connected: true, microphone: true, camera: true, remoteAudio: false })).toBe(false);
    // The microphone alone (dictation, a voice recorder, a camera test) is no call.
    expect(looksLikeCall({ connected: false, microphone: true, camera: true, remoteAudio: false })).toBe(false);
  });

  it("keeps a call going while its connection is open, through silence", () => {
    const quiet = { connected: true, microphone: false, camera: false, remoteAudio: false };
    expect(looksLikeCall(quiet, true)).toBe(true);
    expect(looksLikeCall({ ...quiet, connected: false }, true)).toBe(false);
  });
});

describe("remoteAudioTracks", () => {
  it("lists every live audio track received by open connections, once", () => {
    const first = track("audio");
    const second = track("audio", "live", true);
    const call = connection([{ received: first }, { received: second }, { received: track("video") }]);
    const ended = connection([{ received: track("audio", "ended") }]);
    const closed = connection([{ received: track("audio") }], "closed");
    expect(remoteAudioTracks([call, call, ended, closed])).toEqual([first, second]);
  });
});

describe("CallDetector", () => {
  it("starts a call once it has looked like one for the start delay", () => {
    const detector = new CallDetector(3_000, 8_000);
    expect(detector.observe(true, 0)).toBeNull();
    expect(detector.observe(true, 2_000)).toBeNull();
    expect(detector.observe(true, 3_000)).toBe("started");
    expect(detector.inCall).toBe(true);
    expect(detector.observe(true, 4_000)).toBeNull();
  });

  it("forgets a short blip before the start delay", () => {
    const detector = new CallDetector(3_000, 8_000);
    detector.observe(true, 0);
    detector.observe(false, 1_000);
    expect(detector.observe(true, 3_500)).toBeNull();
    expect(detector.observe(true, 6_500)).toBe("started");
  });

  it("ends a call only after the grace period, so a reconnect does not end it", () => {
    const detector = new CallDetector(3_000, 8_000);
    detector.observe(true, 0);
    detector.observe(true, 3_000);
    expect(detector.observe(false, 4_000)).toBeNull();
    expect(detector.observe(true, 9_000)).toBeNull();
    expect(detector.observe(false, 10_000)).toBeNull();
    expect(detector.observe(false, 17_999)).toBeNull();
    expect(detector.observe(false, 18_000)).toBe("ended");
    expect(detector.inCall).toBe(false);
  });
});
