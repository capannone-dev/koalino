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

/**
 * A 640×360 shared window with one text slide, modelled on the #102 live test: same layout on
 * every slide, only the title and bullet lengths differ. `textWidths` are the title and bullet
 * widths in pixels; `pointer` is the mouse pointer's top-left corner.
 */
function textSlide(textWidths: number[], pointer?: { x: number; y: number }): RgbaImage {
  const image = frame(640, 360, 33);
  paint(image, { left: 0, top: 0, right: 640, bottom: 30 }, [240, 240, 240]);
  paint(image, { left: 100, top: 60, right: 540, bottom: 308 }, [255, 255, 255]);
  paint(image, { left: 100, top: 296, right: 540, bottom: 308 }, [30, 70, 120]);
  textWidths.forEach((width, line) => {
    const top = line === 0 ? 80 : 104 + line * 18;
    paint(image, { left: 125, top, right: 125 + width, bottom: top + (line === 0 ? 14 : 8) }, [20, 40, 70]);
  });
  if (pointer) paint(image, { left: pointer.x, top: pointer.y, right: pointer.x + 8, bottom: pointer.y + 13 }, [0, 0, 0]);
  return image;
}

describe("SlideChangeDetector", () => {
  it("keeps a slide whose layout matches the last one and only its text changed", () => {
    const detector = new SlideChangeDetector();
    const first = signatureOf(textSlide([200, 100, 115, 140]));
    const third = signatureOf(textSlide([150, 95, 100, 105]));
    detector.offer(first);
    expect(detector.offer(first)).toBe(true);
    detector.offer(third);
    expect(detector.offer(third)).toBe(true);
  });

  it("does not keep the same slide again when only the pointer moved", () => {
    const detector = new SlideChangeDetector();
    const words = [200, 100, 115, 140];
    detector.offer(signatureOf(textSlide(words, { x: 300, y: 200 })));
    expect(detector.offer(signatureOf(textSlide(words, { x: 300, y: 200 })))).toBe(true);
    detector.offer(signatureOf(textSlide(words, { x: 420, y: 240 })));
    expect(detector.offer(signatureOf(textSlide(words, { x: 420, y: 240 })))).toBe(false);
  });

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
