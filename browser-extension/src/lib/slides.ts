/**
 * Slide capture from a screen share (Issue #102): finding the slide inside a shared frame, and
 * deciding when the content changed and settled, so each slide is kept once.
 *
 * Works on plain RGBA buffers (`ImageData`'s layout), so it runs in the content script and in
 * tests alike. A shared window often shows the slide letterboxed inside the presenting app: the
 * slide is the largest rectangle with strong edges on all four sides, a slide-like aspect ratio
 * (1.2–2.0), and an even colour just outside. Edges compare each colour channel, so a coloured
 * slide edge on a dark background counts even when its brightness barely changes. When no such
 * rectangle exists (a full-screen slide show), the whole frame is the slide.
 */

/** An RGBA image. */
export interface RgbaImage {
  width: number;
  height: number;
  data: Uint8ClampedArray;
}

/** A rectangle in pixels; `right` and `bottom` are exclusive. */
export interface Box {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** A step this large in any channel is an edge. */
const EDGE_STEP = 40;
/** A row or column is an edge line when this share of it is edge. */
const LINE_SHARE = 0.25;
/** Each side of the slide must be edge along this share of its length. */
const SIDE_SHARE = 0.8;
/** The band outside the slide checked for an even colour. */
const BAND = 8;
/** The largest brightness standard deviation of that band. */
const BAND_DEVIATION = 6;

/** The largest channel step between two pixels. */
function step(data: Uint8ClampedArray, first: number, second: number): number {
  return Math.max(
    Math.abs((data[first] ?? 0) - (data[second] ?? 0)),
    Math.abs((data[first + 1] ?? 0) - (data[second + 1] ?? 0)),
    Math.abs((data[first + 2] ?? 0) - (data[second + 2] ?? 0)),
  );
}

/**
 * Finds the slide inside a shared frame.
 *
 * Inputs: The frame. Outputs: The slide's box, or `null` when the frame shows no letterboxed
 * slide (keep the whole frame). Side effects: None.
 */
export function findSlideBox(image: RgbaImage): Box | null {
  const { width, height, data } = image;
  // horizontal[y][x]: edge between rows y and y+1; vertical[y][x]: between columns x and x+1.
  const horizontal = new Uint8Array((height - 1) * width);
  const vertical = new Uint8Array(height * (width - 1));
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      const here = (y * width + x) * 4;
      if (y + 1 < height && step(data, here, here + width * 4) > EDGE_STEP) {
        horizontal[y * width + x] = 1;
      }
      if (x + 1 < width && step(data, here, here + 4) > EDGE_STEP) {
        vertical[y * (width - 1) + x] = 1;
      }
    }
  }
  const rowShare = (y: number, from: number, to: number) => {
    let sum = 0;
    for (let x = from; x < to; x += 1) sum += horizontal[y * width + x] ?? 0;
    return sum / Math.max(1, to - from);
  };
  const columnShare = (x: number, from: number, to: number) => {
    let sum = 0;
    for (let y = from; y < to; y += 1) sum += vertical[y * (width - 1) + x] ?? 0;
    return sum / Math.max(1, to - from);
  };
  const rows: number[] = [];
  for (let y = 0; y < height - 1; y += 1) if (rowShare(y, 0, width) > LINE_SHARE) rows.push(y);
  const columns: number[] = [];
  for (let x = 0; x < width - 1; x += 1) if (columnShare(x, 0, height) > LINE_SHARE) columns.push(x);

  let best: { area: number; box: Box } | null = null;
  for (const [topIndex, top] of rows.entries()) {
    for (const bottom of rows.slice(topIndex + 1)) {
      if (bottom - top < 0.2 * height) continue;
      for (const [leftIndex, left] of columns.entries()) {
        for (const right of columns.slice(leftIndex + 1)) {
          if (right - left < 0.2 * width) continue;
          const ratio = (right - left) / (bottom - top);
          if (ratio < 1.2 || ratio > 2.0) continue;
          const sides = [
            rowShare(top, left, right),
            rowShare(bottom, left, right),
            columnShare(left, top, bottom),
            columnShare(right, top, bottom),
          ];
          if (Math.min(...sides) < SIDE_SHARE) continue;
          if (!evenBand(image, top, bottom, left, right)) continue;
          const area = (right - left) * (bottom - top);
          if (!best || area > best.area) {
            best = { area, box: { left: left + 1, top: top + 1, right: right + 1, bottom: bottom + 1 } };
          }
        }
      }
    }
  }
  if (!best) return null;
  const { box } = best;
  const coversFrame =
    box.right - box.left >= width - 2 * BAND && box.bottom - box.top >= height - 2 * BAND;
  return coversFrame ? null : box;
}

/** Whether the band just outside the edge lines has one even brightness. */
function evenBand(image: RgbaImage, top: number, bottom: number, left: number, right: number): boolean {
  const { width, height, data } = image;
  const values: number[] = [];
  const add = (x: number, y: number) => {
    const index = (y * width + x) * 4;
    values.push(((data[index] ?? 0) + (data[index + 1] ?? 0) + (data[index + 2] ?? 0)) / 3);
  };
  for (let y = Math.max(top - BAND + 1, 0); y <= top; y += 1) for (let x = left + 1; x <= right; x += 1) add(x, y);
  for (let y = bottom + 1; y < Math.min(bottom + 1 + BAND, height); y += 1) for (let x = left + 1; x <= right; x += 1) add(x, y);
  for (let y = top + 1; y <= bottom; y += 1) {
    for (let x = Math.max(left - BAND + 1, 0); x <= left; x += 1) add(x, y);
    for (let x = right + 1; x < Math.min(right + 1 + BAND, width); x += 1) add(x, y);
  }
  if (values.length === 0) return false;
  const mean = values.reduce((sum, value) => sum + value, 0) / values.length;
  const variance = values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / values.length;
  return Math.sqrt(variance) <= BAND_DEVIATION;
}

/**
 * Columns and rows of a frame signature: fine enough that a changed line of slide text moves
 * several cells (one cell is about 20 pixels of a 1280-pixel share).
 */
const SIGNATURE_WIDTH = 64;
const SIGNATURE_HEIGHT = 36;

/**
 * A small greyscale summary of a frame (64×36 cell averages), cheap to compare.
 *
 * Inputs: The frame. Outputs: The cell brightnesses. Side effects: None.
 */
export function signatureOf(image: RgbaImage): Float32Array {
  const signature = new Float32Array(SIGNATURE_WIDTH * SIGNATURE_HEIGHT);
  const counts = new Uint32Array(signature.length);
  const { width, height, data } = image;
  const stride = Math.max(1, Math.floor(Math.min(width / SIGNATURE_WIDTH, height / SIGNATURE_HEIGHT) / 4));
  for (let y = 0; y < height; y += stride) {
    const cellY = Math.min(SIGNATURE_HEIGHT - 1, Math.floor((y / height) * SIGNATURE_HEIGHT));
    for (let x = 0; x < width; x += stride) {
      const cellX = Math.min(SIGNATURE_WIDTH - 1, Math.floor((x / width) * SIGNATURE_WIDTH));
      const index = (y * width + x) * 4;
      const cell = cellY * SIGNATURE_WIDTH + cellX;
      signature[cell] = (signature[cell] ?? 0) + ((data[index] ?? 0) + (data[index + 1] ?? 0) + (data[index + 2] ?? 0)) / 3;
      counts[cell] = (counts[cell] ?? 0) + 1;
    }
  }
  return signature.map((sum, cell) => sum / Math.max(1, counts[cell] ?? 1));
}

/** Mean absolute difference between two signatures. */
function distance(first: Float32Array, second: Float32Array): number {
  let sum = 0;
  for (let index = 0; index < first.length; index += 1) sum += Math.abs((first[index] ?? 0) - (second[index] ?? 0));
  return sum / first.length;
}

/** Number of cells whose brightness differs by more than `step`. */
function changedCells(first: Float32Array, second: Float32Array, step: number): number {
  let count = 0;
  for (let index = 0; index < first.length; index += 1) {
    if (Math.abs((first[index] ?? 0) - (second[index] ?? 0)) > step) count += 1;
  }
  return count;
}

/**
 * A cell changed when its brightness moved by more than this (video compression noise stays near
 * 2; a line of text appearing or disappearing moves a cell by 15 to 90 in the #102 live test).
 */
const CHANGED_CELL_STEP = 12;
/**
 * Content differs from the last kept slide when at least this many cells changed. The mouse
 * pointer covers at most four cells; slides sharing a layout differed in about 47 in the live test.
 * A mean over the whole frame missed them: text is a small share of a slide.
 */
const CHANGED_CELLS = 8;
/** Two consecutive frames are still when they differ by less than this on average. */
const STILL = 0.8;

/**
 * Decides which frames of a share to keep: one per slide, once it settled (a frame equal to the
 * previous one), and only when it differs from the last kept slide.
 */
export class SlideChangeDetector {
  private kept: Float32Array | null = null;
  private previous: Float32Array | null = null;

  /**
   * Offers the latest frame's signature.
   *
   * Inputs: The signature (sample about once a second). Outputs: Whether to keep this frame.
   * Side effects: Updates the detector.
   */
  offer(signature: Float32Array): boolean {
    const previous = this.previous;
    this.previous = signature;
    if (!previous || distance(previous, signature) >= STILL) return false;
    if (this.kept && changedCells(this.kept, signature, CHANGED_CELL_STEP) < CHANGED_CELLS) return false;
    this.kept = signature;
    return true;
  }

  /** Forgets the kept slide (a new share started). Side effects: Resets the detector. */
  reset(): void {
    this.kept = null;
    this.previous = null;
  }
}
