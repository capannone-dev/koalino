/**
 * Specifies how the page hook finds the remote audio Teams plays (#160): in Firefox an `<audio>`
 * element in the page, in Chrome a muted `<audio>` element never added to the page, whose stream
 * Teams plays through Web Audio.
 */
import { describe, expect, it } from "vitest";
import { StreamLike, observePlayedStreams, remoteCallStream } from "../src/lib/remoteAudio";

/** A stream with one audio track of `label` in `readyState` ("live" by default). */
function stream(label: string, readyState = "live"): StreamLike {
  const track = { label, readyState };
  return { getAudioTracks: () => [track] };
}

describe("observePlayedStreams (#160)", () => {
  /** A media element prototype with a native-like `srcObject` accessor. */
  function mediaPrototype(): { prototype: object; make: () => { srcObject: unknown } } {
    const values = new WeakMap<object, unknown>();
    const prototype = {};
    Object.defineProperty(prototype, "srcObject", {
      configurable: true,
      get(this: object) {
        return values.get(this) ?? null;
      },
      set(this: object, value: unknown) {
        values.set(this, value);
      },
    });
    return { prototype, make: () => Object.create(prototype) as { srcObject: unknown } };
  }

  it("reports elements given a stream, even ones never added to the page", () => {
    const { prototype, make } = mediaPrototype();
    const seen: unknown[] = [];
    observePlayedStreams(prototype, (element) => seen.push(element));
    const detached = make();
    const remote = stream("remote audio");
    detached.srcObject = remote;
    expect(seen).toEqual([detached]);
    // The page still reads back what it set.
    expect(detached.srcObject).toBe(remote);
  });

  it("ignores values that are not streams and never breaks the page's assignment", () => {
    const { prototype, make } = mediaPrototype();
    const seen: unknown[] = [];
    observePlayedStreams(prototype, () => {
      throw new Error("observer failed");
    });
    observePlayedStreams(prototype, (element) => seen.push(element));
    const element = make();
    element.srcObject = null;
    expect(seen).toEqual([]);
    element.srcObject = stream("remote audio");
    expect(element.srcObject).not.toBeNull();
  });

  it("does nothing where there is no srcObject accessor", () => {
    expect(() => observePlayedStreams(undefined, () => undefined)).not.toThrow();
    expect(() => observePlayedStreams({}, () => undefined)).not.toThrow();
  });
});

describe("remoteCallStream (#160)", () => {
  const notMicrophone = () => false;

  it("finds the stream of a detached element (Teams in Chrome)", () => {
    const remote = stream("remote audio");
    expect(remoteCallStream([{ srcObject: null }, { srcObject: remote }], notMicrophone)).toBe(remote);
  });

  it("skips ended streams and the owner's microphone", () => {
    const microphone = stream("Default");
    const [microphoneTrack] = microphone.getAudioTracks();
    const ended = stream("remote audio", "ended");
    const isMicrophone = (track: unknown) => track === microphoneTrack;
    expect(remoteCallStream([{ srcObject: ended }, { srcObject: microphone }], isMicrophone)).toBeNull();
  });

  it("ignores elements showing something other than a stream", () => {
    expect(remoteCallStream([{ srcObject: "blob:x" }, { srcObject: {} }], notMicrophone)).toBeNull();
  });
});
