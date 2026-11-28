/**
 * The extension's icons (#207): the same Material icons (Apache-2.0) the app and the dashboard
 * use, drawn inline so the popup and the options page need no font or network. Each is decorative
 * (`aria-hidden`); the control that holds it carries the words.
 */

/** The SVG path of each icon, from Material Icons. */
export const iconPaths = {
  mic: "M12 14c1.66 0 2.99-1.34 2.99-3L15 5c0-1.66-1.34-3-3-3S9 3.34 9 5v6c0 1.66 1.34 3 3 3zm5.3-3c0 3-2.54 5.1-5.3 5.1S6.7 14 6.7 11H5c0 3.41 2.72 6.23 6 6.72V21h2v-3.28c3.28-.48 6-3.3 6-6.72h-1.7z",
  stop: "M6 6h12v12H6z",
  settings:
    "M19.14 12.94c.04-.3.06-.61.06-.94 0-.32-.02-.64-.07-.94l2.03-1.58a.49.49 0 0 0 .12-.61l-1.92-3.32a.488.488 0 0 0-.59-.22l-2.39.96c-.5-.38-1.03-.7-1.62-.94l-.36-2.54a.484.484 0 0 0-.48-.41h-3.84c-.24 0-.43.17-.47.41l-.36 2.54c-.59.24-1.13.57-1.62.94l-2.39-.96c-.22-.08-.47 0-.59.22L2.74 8.87c-.12.21-.08.47.12.61l2.03 1.58c-.05.3-.09.63-.09.94s.02.64.07.94l-2.03 1.58a.49.49 0 0 0-.12.61l1.92 3.32c.12.22.37.29.59.22l2.39-.96c.5.38 1.03.7 1.62.94l.36 2.54c.05.24.24.41.48.41h3.84c.24 0 .44-.17.47-.41l.36-2.54c.59-.24 1.13-.56 1.62-.94l2.39.96c.22.08.47 0 .59-.22l1.92-3.32c.12-.22.07-.47-.12-.61l-2.01-1.58zM12 15.6c-1.98 0-3.6-1.62-3.6-3.6s1.62-3.6 3.6-3.6 3.6 1.62 3.6 3.6-1.62 3.6-3.6 3.6z",
  check: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z",
  error: "M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm1 15h-2v-2h2v2zm0-4h-2V7h2v6z",
  refresh:
    "M17.65 6.35A7.958 7.958 0 0 0 12 4c-4.42 0-7.99 3.58-7.99 8s3.57 8 7.99 8c3.73 0 6.84-2.55 7.73-6h-2.08A5.99 5.99 0 0 1 12 18c-3.31 0-6-2.69-6-6s2.69-6 6-6c1.66 0 3.14.69 4.22 1.78L13 11h7V4l-2.35 2.35z",
} as const;

/** The names of the available icons. */
export type IconName = keyof typeof iconPaths;

const svgNamespace = "http://www.w3.org/2000/svg";

/**
 * Makes one icon element in the current text colour.
 *
 * Inputs: the icon's name and its size in px (default 18). Outputs: an `<svg>` element.
 * Side effects: none (the caller inserts it).
 */
export function iconElement(name: IconName, size = 18): SVGSVGElement {
  const svg = document.createElementNS(svgNamespace, "svg");
  svg.setAttribute("aria-hidden", "true");
  svg.setAttribute("focusable", "false");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("width", String(size));
  svg.setAttribute("height", String(size));
  svg.setAttribute("fill", "currentColor");
  svg.classList.add("icon");
  const path = document.createElementNS(svgNamespace, "path");
  path.setAttribute("d", iconPaths[name]);
  svg.append(path);
  return svg;
}

/**
 * Sets `element`'s content to an icon followed by `text`, so a label and its icon change together.
 *
 * Side effects: replaces the element's children.
 */
export function setIconLabel(element: Element, name: IconName, text: string): void {
  const label = document.createElement("span");
  label.textContent = text;
  element.replaceChildren(iconElement(name), label);
}

/**
 * Puts the icon named by `data-icon` before the text of every such element under `root`.
 *
 * Side effects: inserts the icons once (an element that has one keeps it).
 */
export function addIcons(root: ParentNode = document): void {
  for (const element of root.querySelectorAll<HTMLElement>("[data-icon]")) {
    const name = element.dataset.icon as IconName | undefined;
    if (!name || !(name in iconPaths) || element.querySelector(":scope > svg.icon")) continue;
    element.prepend(iconElement(name));
  }
}
