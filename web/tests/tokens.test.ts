// Checks that the dashboard's accent follows Koalino's shared tokens (#206): the light and dark
// accent variables in app/styles.css match branding/tokens.json, the logo's purple.

import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { expect, test } from "vitest";

const tokens = JSON.parse(readFileSync(resolve(__dirname, "../../branding/tokens.json"), "utf8")) as {
  light: Record<string, string>;
  dark: Record<string, string>;
};
const css = readFileSync(resolve(__dirname, "../app/styles.css"), "utf8");
const [lightBlock = "", darkBlock = ""] = css.split("@media (prefers-color-scheme: dark)");

/** The value of `--color-<name>` in a block of CSS. */
function variable(block: string, name: string): string | undefined {
  return new RegExp(`--color-${name}:\\s*(#[0-9a-fA-F]+);`).exec(block)?.[1]?.toLowerCase();
}

for (const [scheme, block] of [["light", lightBlock], ["dark", darkBlock]] as const) {
  test(`the ${scheme} scheme uses the shared accent tokens`, () => {
    expect(variable(block, "accent")).toBe(tokens[scheme].accent);
    expect(variable(block, "accent-strong")).toBe(tokens[scheme].accentStrong);
    expect(variable(block, "accent-text")).toBe(tokens[scheme].accentText);
    expect(variable(block, "accent-soft")).toBe(tokens[scheme].accentSoft);
    expect(variable(block, "focus")).toBe(tokens[scheme].focus);
  });
}
