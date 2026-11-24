/**
 * Specifies how the page hook finds the owner's microphone in Teams' call connections (#157):
 * the audio track a connection sends, including connections opened before the hook ran.
 */
import { describe, expect, it, vi } from "vitest";
import {
  ConnectionLike,
  TrackLike,
  claimHookGeneration,
  observeConnections,
  sentMicrophone,
} from "../src/lib/sentMicrophone";

/** A track of `kind` in `readyState` ("live" by default). */
function track(kind: string, readyState = "live"): TrackLike {
  return { kind, readyState };
}

/** A connection whose transceivers send `sent` and receive tracks of `received`'s kind. */
function connection(
  transceivers: { sent: TrackLike | null; received: string; direction?: string }[],
  connectionState = "connected",
): ConnectionLike {
  return {
    connectionState,
    getTransceivers: () =>
      transceivers.map(({ sent, received, direction }) => ({
        sender: { track: sent },
        receiver: { track: track(received) },
        currentDirection: direction ?? "sendrecv",
      })),
  };
}

describe("sentMicrophone", () => {
  it("knows nothing when no connection sends audio", () => {
    expect(sentMicrophone([])).toEqual({ kind: "unknown" });
    expect(sentMicrophone([connection([{ sent: track("video"), received: "video" }])])).toEqual({
      kind: "unknown",
    });
  });

  it("takes the live audio track a connection sends", () => {
    const microphone = track("audio");
    const found = sentMicrophone([
      connection([
        { sent: track("video"), received: "video" },
        { sent: microphone, received: "audio" },
      ]),
    ]);
    expect(found).toEqual({ kind: "track", track: microphone });
  });

  it("counts an audio sender without a live track as muted, never unknown", () => {
    expect(sentMicrophone([connection([{ sent: null, received: "audio" }])])).toEqual({ kind: "muted" });
    expect(sentMicrophone([connection([{ sent: track("audio", "ended"), received: "audio" }])])).toEqual({
      kind: "muted",
    });
  });

  it("ignores receive-only audio and closed connections", () => {
    expect(
      sentMicrophone([connection([{ sent: null, received: "audio", direction: "recvonly" }])]),
    ).toEqual({ kind: "unknown" });
    expect(sentMicrophone([connection([{ sent: track("audio"), received: "audio" }], "closed")])).toEqual({
      kind: "unknown",
    });
  });

  it("prefers a connection that sends a live track over one that is muted", () => {
    const microphone = track("audio");
    const found = sentMicrophone([
      connection([{ sent: null, received: "audio" }]),
      connection([{ sent: microphone, received: "audio" }]),
    ]);
    expect(found).toEqual({ kind: "track", track: microphone });
  });

  it("treats a connection whose transceivers cannot be read as sending nothing", () => {
    const broken: ConnectionLike = {
      connectionState: "connected",
      getTransceivers: () => {
        throw new Error("gone");
      },
    };
    expect(sentMicrophone([broken])).toEqual({ kind: "unknown" });
  });
});

describe("observeConnections", () => {
  /** A stand-in for `RTCPeerConnection` with the methods Teams calls. */
  class FakeConnection {
    getStats(selector?: string): Promise<string> {
      return Promise.resolve(`stats ${selector ?? "all"}`);
    }
    addTrack(name: string): string {
      return `sender ${name}`;
    }
    addTransceiver(kind: string): string {
      return `transceiver ${kind}`;
    }
  }

  it("reports connections created before the patch when Teams next calls them", async () => {
    const early = new FakeConnection();
    const seen = vi.fn();
    observeConnections(FakeConnection.prototype, seen);

    await expect(early.getStats("audio")).resolves.toBe("stats audio");
    expect(early.addTrack("mic")).toBe("sender mic");
    expect(early.addTransceiver("audio")).toBe("transceiver audio");
    expect(seen).toHaveBeenCalledTimes(3);
    expect(seen).toHaveBeenNthCalledWith(1, early);
  });

  it("never lets an observer's failure reach the page", async () => {
    const connectionToWatch = new FakeConnection();
    observeConnections(FakeConnection.prototype, () => {
      throw new Error("observer broke");
    });
    await expect(connectionToWatch.getStats()).resolves.toBe("stats all");
  });

  it("does nothing when the prototype is missing", () => {
    expect(() => observeConnections(undefined, vi.fn())).not.toThrow();
  });
});

describe("claimHookGeneration", () => {
  it("lets only the newest hook in a page act", () => {
    const page = {};
    const first = claimHookGeneration(page);
    expect(first()).toBe(true);

    const second = claimHookGeneration(page);
    expect(second()).toBe(true);
    expect(first()).toBe(false);
  });

  it("keeps separate pages apart", () => {
    const one = claimHookGeneration({});
    claimHookGeneration({});
    expect(one()).toBe(true);
  });
});
