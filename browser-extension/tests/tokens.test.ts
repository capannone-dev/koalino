/**
 * Checks that the extension's colours follow Koalino's shared tokens (#206): the pages' stylesheet
 * and the in-page banner use the accent values of `branding/tokens.json`.
 */
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const tokens = JSON.parse(readFileSync(resolve(__dirname, "../../branding/tokens.json"), "utf8")) as {
  light: Record<string, string>;
  dark: Record<string, string>;
};
const css = readFileSync(resolve(__dirname, "../static/koalino.css"), "utf8");
const [lightBlock = "", darkBlock = ""] = css.split("@media (prefers-color-scheme: dark)");

/** The value of `--name` in a block of CSS. */
function variable(block: string, name: string): string | undefined {
  return new RegExp(`--${name}:\\s*(#[0-9a-fA-F]+);`).exec(block)?.[1]?.toLowerCase();
}

describe("koalino.css", () => {
  for (const [scheme, block] of [["light", lightBlock], ["dark", darkBlock]] as const) {
    it(`uses the ${scheme} accent tokens`, () => {
      expect(variable(block, "accent")).toBe(tokens[scheme].accent);
      expect(variable(block, "accent-strong")).toBe(tokens[scheme].accentStrong);
      expect(variable(block, "accent-text")).toBe(tokens[scheme].accentText);
      expect(variable(block, "focus")).toBe(tokens[scheme].focus);
    });
  }

  it("the in-page banner's main button uses the dark accent", () => {
    const banner = readFileSync(resolve(__dirname, "../src/content/banner.ts"), "utf8");
    expect(banner).toContain(`button.primary { background: ${tokens.dark.accent}; }`);
  });
});
