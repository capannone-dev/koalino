// Verifies the light and dark colour tokens: every token has a dark value, the page declares both
// colour schemes, and components use tokens instead of palette colours that would not switch.

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { expect, test } from "vitest";

const webRoot = join(__dirname, "..");
const styles = readFileSync(join(webRoot, "app", "styles.css"), "utf8");

/** Returns the custom properties declared inside the first block that follows `marker`. */
function tokensAfter(marker: string): Set<string> {
  const start = styles.indexOf(marker);
  expect(start, `styles.css must contain ${marker}`).toBeGreaterThanOrEqual(0);
  const block = styles.slice(start, styles.indexOf("}", start));
  return new Set(Array.from(block.matchAll(/(--color-[a-z-]+)\s*:/g), (match) => match[1]));
}

/** Lists the .tsx sources under `directory`, skipping tests. */
function sources(directory: string): string[] {
  return readdirSync(directory).flatMap((name) => {
    const path = join(directory, name);
    if (statSync(path).isDirectory()) return sources(path);
    return name.endsWith(".tsx") && !name.includes(".test.") ? [path] : [];
  });
}

test("every light colour token has a dark value", () => {
  const light = tokensAfter("@theme");
  const dark = tokensAfter("@media (prefers-color-scheme: dark)");

  expect(light.size).toBeGreaterThan(5);
  expect([...light].filter((token) => !dark.has(token))).toEqual([]);
});

test("the page tells the browser it supports both colour schemes", () => {
  expect(styles).toMatch(/color-scheme:\s*light dark/);
});

test("components use theme tokens, not fixed light colours", () => {
  const fixedColour =
    /\b(?:bg|text|border|divide|ring|outline)-(?:white|slate-(?:[1-7]00|50)|blue-\d+|green-\d+|red-\d+|emerald-\d+|amber-\d+)\b/g;
  const offenders = [...sources(join(webRoot, "app")), ...sources(join(webRoot, "components"))].flatMap((path) =>
    Array.from(readFileSync(path, "utf8").matchAll(fixedColour), (match) => `${path.slice(webRoot.length + 1)}: ${match[0]}`)
  );

  expect(offenders).toEqual([]);
});
