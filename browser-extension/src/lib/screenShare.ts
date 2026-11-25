/**
 * Finds the shared screen among a call's videos on any website (#174).
 *
 * Teams marks its share stream (`findShareVideo`); other services do not, and each lays out its
 * videos its own way. So the page hook looks at every video the page plays and asks two things:
 *
 * - Does a frame look like screen content? Slides, documents and code have large areas of exactly
 *   one colour, a few colours covering most of the frame, sharp text edges, and little skin.
 *   A camera has sensor noise and soft gradients; a camera-off tile has no edges at all.
 * - Does it hold still? A shared screen stays the same frame for seconds; a camera never does.
 *
 * The screen the owner shares (`getDisplayMedia`) wins outright. The owner's own camera preview
 * never counts. Once chosen, a share is kept while it still looks like screen content, even when
 * a video plays inside it, and let go when it no longer does or is gone. Of several still screens
 * the largest is taken: the call's main stage rather than a thumbnail.
 *
 * Works on downscaled RGBA frames (`RgbaImage`), so it runs in tests. Nothing is logged.
 */
import { RgbaImage } from "./slides";

/** What a frame looks like, as shares of its pixels (0–1). */
export interface ScreenContentStats {
  /** Pixels whose right and lower neighbours differ by at most `FLAT_STEP`. */
  flatShare: number;
  /** Pixels with a step of at least `EDGE_STEP` to a neighbour (text, lines, borders). */
  edgeShare: number;
  /** Pixels in the two most common colours (16 levels per channel). */
  dominantShare: number;
  /** Pixels of a skin tone. */
  skinShare: number;
}

/** Neighbours this close are the same colour (video compression leaves at most this). */
const FLAT_STEP = 2;
/** A step this large between neighbours is an edge. */
const EDGE_STEP = 60;
/** Screen content: at least this share is flat… */
const MINIMUM_FLAT_SHARE = 0.5;
/** …at least this share is in its two main colours… */
const MINIMUM_DOMINANT_SHARE = 0.4;
/** …at least this share is edge (some text or lines)… */
const MINIMUM_EDGE_SHARE = 0.003;
/** …and at most this share is skin. */
const MAXIMUM_SKIN_SHARE = 0.2;
/** Two signatures this close on average (0–255) are the same picture. */
const STILL_DIFFERENCE = 1.5;

/** The largest channel step between the pixels at byte offsets `first` and `second`. */
function step(data: Uint8ClampedArray, first: number, second: number): number {
  return Math.max(
    Math.abs((data[first] ?? 0) - (data[second] ?? 0)),
    Math.abs((data[first + 1] ?? 0) - (data[second + 1] ?? 0)),
    Math.abs((data[first + 2] ?? 0) - (data[second + 2] ?? 0)),
  );
}

/** Whether a colour is a skin tone (a common RGB rule for daylight skin). */
function isSkin(red: number, green: number, blue: number): boolean {
  return (
    red > 95 &&
    green > 40 &&
    blue > 20 &&
    red > green &&
    red > blue &&
    red - Math.min(green, blue) > 15 &&
    Math.abs(red - green) > 15
  );
}

/**
 * Measures what a frame looks like.
 *
 * Inputs: The frame, best downscaled to about 160×90 (text strokes still make edges there).
 * Outputs: The shares; all 0 for an image smaller than 2×2. Side effects: None.
 */
export function screenContentStats(image: RgbaImage): ScreenContentStats {
  const { width, height, data } = image;
  if (width < 2 || height < 2) return { flatShare: 0, edgeShare: 0, dominantShare: 0, skinShare: 0 };
  let flat = 0;
  let edge = 0;
  let skin = 0;
  const colourCounts = new Uint32Array(4096);
  for (let y = 0; y < height - 1; y += 1) {
    for (let x = 0; x < width - 1; x += 1) {
      const offset = (y * width + x) * 4;
      const right = step(data, offset, offset + 4);
      const down = step(data, offset, offset + width * 4);
      if (right <= FLAT_STEP && down <= FLAT_STEP) flat += 1;
      if (Math.max(right, down) >= EDGE_STEP) edge += 1;
      const red = data[offset] ?? 0;
      const green = data[offset + 1] ?? 0;
      const blue = data[offset + 2] ?? 0;
      if (isSkin(red, green, blue)) skin += 1;
      const bin = ((red >> 4) << 8) | ((green >> 4) << 4) | (blue >> 4);
      colourCounts[bin] = (colourCounts[bin] ?? 0) + 1;
    }
  }
  let first = 0;
  let second = 0;
  for (const count of colourCounts) {
    if (count > first) {
      second = first;
      first = count;
    } else if (count > second) {
      second = count;
    }
  }
  const pixels = (width - 1) * (height - 1);
  return {
    flatShare: flat / pixels,
    edgeShare: edge / pixels,
    dominantShare: (first + second) / pixels,
    skinShare: skin / pixels,
  };
}

/**
 * Whether a frame looks like screen content (slides, a document, code) rather than a camera.
 *
 * Inputs: The frame's stats. Outputs: `true` for screen content. Side effects: None.
 */
export function looksLikeScreen(stats: ScreenContentStats): boolean {
  return (
    stats.flatShare >= MINIMUM_FLAT_SHARE &&
    stats.dominantShare >= MINIMUM_DOMINANT_SHARE &&
    stats.edgeShare >= MINIMUM_EDGE_SHARE &&
    stats.skinShare <= MAXIMUM_SKIN_SHARE
  );
}

/** One video the page plays, as the hook measured it this round. */
export interface ShareCandidate<Video> {
  video: Video;
  /** Its picture's size in pixels. */
  width: number;
  height: number;
  /** It shows a screen the owner shares (`getDisplayMedia`). */
  ownShare: boolean;
  /** It shows the owner's own camera (`getUserMedia`). */
  localCamera: boolean;
  /** What its frame looks like, or null when it could not be read. */
  stats: ScreenContentStats | null;
  /** Its frame's `signatureOf`, to tell whether it holds still. */
  signature: Float32Array;
}

/** Mean absolute difference between two signatures; Infinity when their sizes differ. */
function difference(first: Float32Array, second: Float32Array): number {
  if (first.length !== second.length || first.length === 0) return Number.POSITIVE_INFINITY;
  let total = 0;
  for (let index = 0; index < first.length; index += 1) {
    total += Math.abs((first[index] ?? 0) - (second[index] ?? 0));
  }
  return total / first.length;
}

/** Chooses the shared screen, round after round (about one a second). */
export class ShareTracker<Video> {
  private chosen: Video | null = null;
  private readonly lastSignature = new Map<Video, Float32Array>();
  /** Consecutive rounds each video held still. */
  private readonly stillRounds = new Map<Video, number>();

  /**
   * Takes this round's videos.
   *
   * Inputs: Every video the page plays now, measured.
   * Outputs: The video showing the shared screen, or null when none does.
   * Side effects: Remembers each video's frame and stillness until it is no longer offered.
   */
  choose(candidates: readonly ShareCandidate<Video>[]): Video | null {
    for (const candidate of candidates) {
      const previous = this.lastSignature.get(candidate.video);
      const still = previous !== undefined && difference(previous, candidate.signature) < STILL_DIFFERENCE;
      this.stillRounds.set(candidate.video, still ? (this.stillRounds.get(candidate.video) ?? 0) + 1 : 0);
      this.lastSignature.set(candidate.video, candidate.signature);
    }
    const offered = new Set(candidates.map((candidate) => candidate.video));
    for (const video of [...this.lastSignature.keys()]) {
      if (offered.has(video)) continue;
      this.lastSignature.delete(video);
      this.stillRounds.delete(video);
    }

    const own = candidates.find((candidate) => candidate.ownShare);
    if (own) {
      this.chosen = own.video;
      return own.video;
    }
    const screens = candidates.filter(
      (candidate) => !candidate.localCamera && candidate.stats !== null && looksLikeScreen(candidate.stats),
    );
    if (this.chosen !== null && screens.some((candidate) => candidate.video === this.chosen)) return this.chosen;
    const settled = screens
      .filter((candidate) => (this.stillRounds.get(candidate.video) ?? 0) >= 1)
      .sort((first, second) => second.width * second.height - first.width * first.height);
    this.chosen = settled[0]?.video ?? null;
    return this.chosen;
  }
}
