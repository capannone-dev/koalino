/**
 * The extension's text in the browser's language (Issue #124). Messages live in `src/locales`;
 * the build turns them into each browser's `_locales/<lang>/messages.json`, which
 * `chrome.i18n.getMessage` reads. Outside the extension (tests), and for a message a language
 * lacks, the English text is used.
 */
import english from "../locales/en.json";

/** The name of a message; English defines them all. */
export type MessageKey = keyof typeof english;
/** Values for a message's `{name}` placeholders. */
export type MessageParams = Readonly<Record<string, string | number>>;

/**
 * The placeholder names of a message, in order of first appearance: the order of the
 * substitutions `chrome.i18n.getMessage` receives.
 *
 * Inputs: An English message. Outputs: Its `{name}` placeholders. Side effects: None.
 */
export function placeholderNames(text: string): string[] {
  return [...new Set([...text.matchAll(/\{([A-Za-z0-9_]+)\}/g)].map((match) => match[1] ?? ""))];
}

/** The browser's `i18n` API when running inside the extension, else undefined. */
function browserI18n(): typeof chrome.i18n | undefined {
  return (globalThis as { chrome?: typeof chrome }).chrome?.i18n;
}

/**
 * Words a message in the browser's language.
 *
 * Inputs: The message key and its placeholder values. Outputs: The translated text, or English
 * with the values filled in when the browser has none. Side effects: None.
 */
export function msg(key: MessageKey, params: MessageParams = {}): string {
  const englishText: string = english[key];
  const values = placeholderNames(englishText).map((name) => String(params[name] ?? ""));
  const translated = browserI18n()?.getMessage(key, values);
  if (translated) return translated;
  return englishText.replace(/\{([A-Za-z0-9_]+)\}/g, (whole, name: string) =>
    name in params ? String(params[name]) : whole,
  );
}

/**
 * The writing direction of the language the messages are shown in, for the popup and the
 * overlay. Read from the messages themselves, so it always matches the text (the browser's own
 * UI-language report can differ from the locale it picked for the extension).
 *
 * Inputs: None. Outputs: `rtl` for Arabic, else `ltr`. Side effects: None.
 */
export function textDirection(): "ltr" | "rtl" {
  return msg("textDirection") === "rtl" ? "rtl" : "ltr";
}

/**
 * The language tag of the messages shown, for the popup's `lang`.
 *
 * Inputs: None. Outputs: For example `it` or `pt-BR`; `en` outside the extension. Side effects: None.
 */
export function uiLanguage(): string {
  return msg("languageTag");
}
