/**
 * Specifies how the page hook finds a shared screen among the videos of any call (#174): what a
 * frame of screen content looks like, and which video is the share, steadily, while a call goes on.
 */
import { describe, expect, it } from "vitest";
import { RgbaImage } from "../src/lib/slides";
import { ShareCandidate, ShareTracker, looksLikeScreen, screenContentStats } from "../src/lib/screenShare";

const WIDTH = 160;
const HEIGHT = 90;

/** An image filled by `colour(x, y)`. */
function image(colour: (x: number, y: number) => [number, number, number]): RgbaImage {
  const data = new Uint8ClampedArray(WIDTH * HEIGHT * 4);
  for (let y = 0; y < HEIGHT; y += 1) {
    for (let x = 0; x < WIDTH; x += 1) {
      const [red, green, blue] = colour(x, y);
      data.set([red, green, blue, 255], (y * WIDTH + x) * 4);
    }
  }
  return { width: WIDTH, height: HEIGHT, data };
}

/** A repeatable pseudo-random number in [0, 1). */
function noise(x: number, y: number, seed = 1): number {
  const value = Math.sin(x * 12.9898 + y * 78.233 + seed * 37.719) * 43_758.5453;
  return value - Math.floor(value);
}

/** A slide: white, a blue title bar, and lines of dark text. */
const slide = image((x, y) => {
  if (y < 14) return [40, 80, 160];
  const inLine = y >= 24 && (y - 24) % 10 < 4 && x >= 10 && x < 140;
  if (inLine && x % 3 !== 0) return [30, 30, 30];
  return [255, 255, 255];
});

/** A code editor: dark background with coloured text. */
const editor = image((x, y) => {
  if (y % 6 < 3 && x > 8 && x < 20 + ((y * 7) % 100) && x % 2 === 0) return [200, 180, 90];
  return [30, 30, 36];
});

/** A webcam: lit wall with sensor noise, a face-coloured blob. */
const webcam = image((x, y) => {
  const grain = Math.round(noise(x, y) * 10);
  const inFace = (x - 80) ** 2 / 900 + (y - 45) ** 2 / 600 < 1;
  if (inFace) return [200 + grain, 150 + grain, 120 + grain];
  const wall = 120 + Math.round((x + y) / 4);
  return [wall + grain, wall + grain - 6, wall + grain - 12];
});

/** A video tile with the camera off: black. */
const black = image(() => [0, 0, 0]);

describe("looksLikeScreen", () => {
  it("takes slides and a code editor for screen content", () => {
    expect(looksLikeScreen(screenContentStats(slide))).toBe(true);
    expect(looksLikeScreen(screenContentStats(editor))).toBe(true);
  });

  it("does not take a camera or an empty tile for one", () => {
    expect(looksLikeScreen(screenContentStats(webcam))).toBe(false);
    expect(looksLikeScreen(screenContentStats(black))).toBe(false);
  });
});

/** A candidate video `id` showing `frame`, `width` pixels wide. */
function candidate(
  id: string,
  frame: RgbaImage,
  options: { width?: number; own?: boolean; localCamera?: boolean; signature?: number[] } = {},
): ShareCandidate<string> {
  const width = options.width ?? 1280;
  return {
    video: id,
    width,
    height: Math.round((width * 9) / 16),
    ownShare: options.own ?? false,
    localCamera: options.localCamera ?? false,
    stats: screenContentStats(frame),
    signature: Float32Array.from(options.signature ?? [10, 20, 30]),
  };
}

describe("ShareTracker", () => {
  it("prefers the screen the owner shares", () => {
    const tracker = new ShareTracker<string>();
    expect(tracker.choose([candidate("camera", webcam), candidate("mine", webcam, { own: true })])).toBe("mine");
  });

  it("takes a remote video as the share once its screen content has held still", () => {
    const tracker = new ShareTracker<string>();
    const videos = () => [candidate("camera", webcam, { signature: [Math.random() * 90] }), candidate("share", slide)];
    expect(tracker.choose(videos())).toBeNull();
    expect(tracker.choose(videos())).toBe("share");
  });

  it("never takes the owner's own camera preview, nor a moving camera", () => {
    const tracker = new ShareTracker<string>();
    let brightness = 0;
    const videos = () => {
      brightness += 40;
      return [candidate("preview", slide, { localCamera: true }), candidate("camera", webcam, { signature: [brightness] })];
    };
    for (let round = 0; round < 5; round += 1) expect(tracker.choose(videos())).toBeNull();
  });

  it("keeps the chosen share while it plays a video, and lets it go once it is gone", () => {
    const tracker = new ShareTracker<string>();
    tracker.choose([candidate("share", slide)]);
    expect(tracker.choose([candidate("share", slide)])).toBe("share");
    // A video playing inside the share moves, but it is still screen content.
    expect(tracker.choose([candidate("share", slide, { signature: [90, 0, 4] })])).toBe("share");
    expect(tracker.choose([candidate("camera", webcam)])).toBeNull();
  });

  it("takes the largest of several still screens", () => {
    const tracker = new ShareTracker<string>();
    const videos = () => [candidate("thumbnail", editor, { width: 320 }), candidate("stage", slide, { width: 1920 })];
    tracker.choose(videos());
    expect(tracker.choose(videos())).toBe("stage");
  });
});
