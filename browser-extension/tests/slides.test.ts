/**
 * Specifies slide capture from a screen share: cropping a frame to the slide, and keeping a
 * frame once the content changed and then held still.
 */
import { describe, expect, it } from "vitest";
import { RgbaImage, SlideChangeDetector, findSlideBox, signatureOf } from "../src/lib/slides";

/** A frame filled with one grey level. */
function frame(width: number, height: number, level: number): RgbaImage {
  const data = new Uint8ClampedArray(width * height * 4);
  for (let index = 0; index < width * height; index += 1) {
    data.set([level, level, level, 255], index * 4);
  }
  return { width, height, data };
}

/** Paints a rectangle in `image`. */
function paint(
  image: RgbaImage,
  box: { left: number; top: number; right: number; bottom: number },
  rgb: [number, number, number],
): void {
  for (let y = box.top; y < box.bottom; y += 1) {
    for (let x = box.left; x < box.right; x += 1) {
      image.data.set([...rgb, 255], (y * image.width + x) * 4);
    }
  }
}

/** A shared window: a title bar, a dark letterbox and a 16:9 slide with a blue bottom bar. */
function sharedWindow(): RgbaImage {
  const image = frame(400, 300, 33);
  paint(image, { left: 0, top: 0, right: 400, bottom: 30 }, [240, 240, 240]);
  paint(image, { left: 60, top: 80, right: 380, bottom: 260 }, [255, 255, 255]);
  paint(image, { left: 60, top: 248, right: 380, bottom: 260 }, [30, 70, 120]);
  paint(image, { left: 90, top: 100, right: 250, bottom: 120 }, [20, 60, 110]);
  return image;
}

describe("findSlideBox", () => {
  it("finds the slide inside a letterboxed window, including a dark bottom bar", () => {
    expect(findSlideBox(sharedWindow())).toEqual({ left: 60, top: 80, right: 380, bottom: 260 });
  });

  it("returns null when the slide fills the frame", () => {
    const fullScreen = frame(320, 180, 255);
    paint(fullScreen, { left: 20, top: 20, right: 200, bottom: 40 }, [0, 0, 0]);
    expect(findSlideBox(fullScreen)).toBeNull();
  });
});

describe("SlideChangeDetector", () => {
  it("keeps the first frame once it holds still, then each changed and settled frame", () => {
    const detector = new SlideChangeDetector();
    const first = signatureOf(sharedWindow());
    const changed = sharedWindow();
    paint(changed, { left: 90, top: 140, right: 360, bottom: 230 }, [0, 0, 0]);
    const second = signatureOf(changed);

    expect(detector.offer(first)).toBe(false);
    expect(detector.offer(first)).toBe(true);
    expect(detector.offer(first)).toBe(false);
    expect(detector.offer(second)).toBe(false);
    expect(detector.offer(second)).toBe(true);
    expect(detector.offer(second)).toBe(false);
  });

  it("does not keep a frame while it is still moving", () => {
    const detector = new SlideChangeDetector();
    const levels = [0, 60, 120, 180, 240];
    const kept = levels.map((level) => detector.offer(signatureOf(frame(64, 36, level))));
    expect(kept.every((wasKept) => !wasKept)).toBe(true);
  });
});
