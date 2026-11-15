/**
 * Converts the extension's message sources (`src/locales/<lang>.json`, flat `key → text` with
 * `{name}` placeholders) into the WebExtension `_locales/<dir>/messages.json` format that
 * `chrome.i18n.getMessage` reads (Issue #124). Used by `build.mjs` (Node runs it with its
 * built-in type stripping) and by the tests, so it imports nothing.
 */

/** One entry of a browser `messages.json`. */
export interface ChromeMessage {
  message: string;
  placeholders?: Record<string, { content: string }>;
}

/** The `{name}` placeholders of a message, in order of first appearance. */
function namesIn(text: string): string[] {
  return [...new Set([...text.matchAll(/\{([A-Za-z0-9_]+)\}/g)].map((match) => match[1] ?? ""))];
}

/**
 * Builds one language's `messages.json` content.
 *
 * Inputs: The language's messages and the English ones, whose placeholder order fixes the
 * substitution numbers (`msg` passes values in that order). Outputs: The browser format, with
 * `{name}` written as `$NAME$` and literal dollars doubled. Side effects: None.
 */
export function chromeMessages(
  messages: Readonly<Record<string, string>>,
  english: Readonly<Record<string, string>>,
): Record<string, ChromeMessage> {
  const converted: Record<string, ChromeMessage> = {};
  for (const [key, text] of Object.entries(messages)) {
    const order = namesIn(english[key] ?? text);
    const message = text
      .split("$")
      .join("$$")
      .replace(/\{([A-Za-z0-9_]+)\}/g, (_whole, name: string) => "$" + name.toUpperCase() + "$");
    if (order.length === 0) {
      converted[key] = { message };
      continue;
    }
    const placeholders: Record<string, { content: string }> = {};
    order.forEach((name, index) => {
      placeholders[name] = { content: "$" + String(index + 1) };
    });
    converted[key] = { message, placeholders };
  }
  return converted;
}

/**
 * The `_locales` folder of one of the extension's languages.
 *
 * Inputs: A source language code (`pt`, `zh`, `it`…). Outputs: The browser's locale name, with
 * Portuguese as Brazilian and Chinese as Simplified, matching the app's wording. Side effects: None.
 */
export function chromeLocaleDirectory(code: string): string {
  if (code === "pt") return "pt_BR";
  if (code === "zh") return "zh_CN";
  return code;
}
