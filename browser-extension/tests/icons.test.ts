// @vitest-environment jsdom
// Specifies the extension's icons (#207): Material paths drawn inline, decorative, beside words.

import { expect, test } from "vitest";
import { addIcons, iconElement, iconPaths, setIconLabel } from "../src/lib/icons";

test("an icon is a decorative 24-unit SVG in the text colour", () => {
  const svg = iconElement("mic", 20);
  expect(svg.getAttribute("aria-hidden")).toBe("true");
  expect(svg.getAttribute("viewBox")).toBe("0 0 24 24");
  expect(svg.getAttribute("width")).toBe("20");
  expect(svg.getAttribute("fill")).toBe("currentColor");
  expect(svg.querySelector("path")?.getAttribute("d")).toBe(iconPaths.mic);
});

test("data-icon puts the icon before the words once, keeping the words", () => {
  document.body.innerHTML = '<a data-icon="settings">Settings</a><button data-icon="nope">Go</button>';
  addIcons();
  addIcons();
  const link = document.querySelector("a") as HTMLElement;
  expect(link.querySelectorAll("svg")).toHaveLength(1);
  expect(link.firstElementChild?.tagName.toLowerCase()).toBe("svg");
  expect(link.textContent).toBe("Settings");
  expect(document.querySelector("button svg")).toBeNull();
});

test("a label and its icon change together", () => {
  const button = document.createElement("button");
  setIconLabel(button, "mic", "Record this tab");
  setIconLabel(button, "stop", "Stop recording this tab");
  expect(button.querySelectorAll("svg")).toHaveLength(1);
  expect(button.querySelector("path")?.getAttribute("d")).toBe(iconPaths.stop);
  expect(button.textContent).toBe("Stop recording this tab");
});
