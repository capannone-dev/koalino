/**
 * Finding the remote audio Teams plays a call through (#102, #160).
 *
 * Teams mixes every remote participant into one stream and hands it to an `<audio>` element. In
 * Firefox that element is in the page. In Chrome it is a muted element never added to the page
 * (Chrome passes a remote WebRTC track to Web Audio only while a media element holds it), and
 * Teams plays the stream through Web Audio. So the page hook notes every element given a stream,
 * in the page or not, and taps the live one that is not the owner's microphone.
 *
 * Structural types keep this testable without a browser.
 */

/** The part of a `MediaStreamTrack` used here. */
export interface TrackLike {
  readyState: string;
}

/** The part of a `MediaStream` used here. */
export interface StreamLike {
  getAudioTracks(): TrackLike[];
}

/** Whether `value` looks like a `MediaStream`. */
function isStream(value: unknown): value is StreamLike {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { getAudioTracks?: unknown }).getAudioTracks === "function"
  );
}

/**
 * Reports every media element the page gives a stream.
 *
 * Inputs: `HTMLMediaElement.prototype` (or undefined where there is none); `played`, called with
 * the element after the page assigned a stream to its `srcObject`.
 * Outputs: None.
 * Side effects: Replaces the prototype's `srcObject` accessor with one that stores the value as
 * before and then calls `played`. A throwing `played` is swallowed, so the page's assignment
 * always takes effect. Without a `srcObject` accessor nothing changes.
 */
export function observePlayedStreams<Element extends object>(
  prototype: Element | undefined,
  played: (element: Element) => void,
): void {
  if (!prototype) return;
  const original = Object.getOwnPropertyDescriptor(prototype, "srcObject");
  const { get, set } = original ?? {};
  if (!original || !get || !set) return;
  Object.defineProperty(prototype, "srcObject", {
    configurable: true,
    enumerable: original.enumerable ?? false,
    get(this: Element): unknown {
      return get.call(this);
    },
    set(this: Element, value: unknown): void {
      set.call(this, value);
      if (!isStream(value)) return;
      try {
        played(this);
      } catch {
        // Never disturb the page.
      }
    },
  });
}

/**
 * The stream of remote call audio among media elements.
 *
 * Inputs: The elements (in the page or not; any order) and `isMicrophone`, which says whether a
 * track is the owner's microphone.
 * Outputs: The first element's stream with a live audio track that is not the microphone, or
 * `null` when none has one.
 * Side effects: None.
 */
export function remoteCallStream<Stream extends StreamLike = StreamLike>(
  elements: Iterable<{ srcObject: unknown }>,
  isMicrophone: (track: TrackLike) => boolean,
): Stream | null {
  for (const element of elements) {
    const stream = element.srcObject;
    if (!isStream(stream)) continue;
    if (stream.getAudioTracks().some((track) => track.readyState === "live" && !isMicrophone(track))) {
      return stream as Stream;
    }
  }
  return null;
}
