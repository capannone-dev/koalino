// The web dashboard's translations (Issue #124): the ten languages, picking one from the owner's
// choice or the browser's, and `Translate` functions that word a message in a language. Shared by
// server pages (`server.ts`), client components (`client.tsx`) and the plain helpers in `lib/`.

import ar from "./messages/ar.json";
import de from "./messages/de.json";
import en from "./messages/en.json";
import es from "./messages/es.json";
import fr from "./messages/fr.json";
import it from "./messages/it.json";
import ja from "./messages/ja.json";
import pt from "./messages/pt.json";
import ru from "./messages/ru.json";
import zh from "./messages/zh.json";

/** A message: plain text, or plural forms keyed `=0`, `=1`, `=2` or by `Intl.PluralRules` category. */
export type Message = string | Readonly<Record<string, string>>;
/** The name of a message; English defines them all. */
export type MessageKey = keyof typeof en;
/** Values put into a message's `{name}` placeholders; `count` also picks a plural form. */
export type Params = Readonly<Record<string, string | number>>;
/** Words one message in one language. */
export type Translate = (key: MessageKey, params?: Params) => string;

/** The languages the dashboard speaks, each named in itself, in Settings order. */
export const languages = [
  { code: "en", name: "English" },
  { code: "it", name: "Italiano" },
  { code: "es", name: "Español" },
  { code: "fr", name: "Français" },
  { code: "de", name: "Deutsch" },
  { code: "pt", name: "Português" },
  { code: "zh", name: "中文（简体）" },
  { code: "ja", name: "日本語" },
  { code: "ru", name: "Русский" },
  { code: "ar", name: "العربية" }
] as const;
export type LanguageCode = (typeof languages)[number]["code"];

/** The cookie holding the owner's choice; absent means "follow the browser". */
export const languageCookie = "koalino-language";

/** Every language's messages; the parity test checks they all have English's keys. */
export const dictionaries: Readonly<Record<LanguageCode, Readonly<Record<string, Message>>>> = {
  en, it, es, fr, de, pt, zh, ja, ru, ar
};

/**
 * Tells whether a string is one of the dashboard's language codes.
 *
 * Inputs: any string or nothing. Outputs: true for `en`, `it`, … Side effects: none.
 */
export function isLanguageCode(code: string | null | undefined): code is LanguageCode {
  return languages.some((language) => language.code === code);
}

/**
 * Picks the language to show: the owner's choice if valid, else the first browser language the
 * dashboard speaks (`it-IT` matches `it`, any `zh` matches Simplified Chinese), else English.
 *
 * Inputs: the cookie's value and the `Accept-Language` header, either possibly missing.
 * Outputs: a language code. Side effects: none.
 */
export function matchLanguage(choice: string | null | undefined, acceptLanguage: string | null | undefined): LanguageCode {
  if (isLanguageCode(choice)) return choice;
  const ranked = (acceptLanguage ?? "")
    .split(",")
    .map((part, index) => {
      const [tag, ...options] = part.trim().split(";");
      const quality = options.map((option) => option.trim()).find((option) => option.startsWith("q="));
      return { base: tag.trim().toLowerCase().split("-")[0], quality: quality ? Number(quality.slice(2)) || 0 : 1, index };
    })
    .filter((entry) => entry.base && entry.quality > 0)
    .sort((a, b) => b.quality - a.quality || a.index - b.index);
  return ranked.map((entry) => entry.base).find(isLanguageCode) ?? "en";
}

/**
 * The writing direction of a language, for the page's `dir`.
 *
 * Inputs: a language code. Outputs: `rtl` for Arabic, `ltr` otherwise. Side effects: none.
 */
export function textDirection(language: LanguageCode): "ltr" | "rtl" {
  return language === "ar" ? "rtl" : "ltr";
}

/**
 * Builds the function that words messages in one language. A message missing from that language
 * falls back to English; a missing placeholder value is left as `{name}`.
 *
 * Inputs: a language code. Outputs: a `Translate`. Side effects: none.
 */
export function translator(language: LanguageCode): Translate {
  const dictionary = dictionaries[language];
  const plurals = new Intl.PluralRules(language);
  return (key, params) => {
    const message: Message = dictionary[key] ?? (en as Record<string, Message>)[key] ?? key;
    let text: string;
    if (typeof message === "string") text = message;
    else {
      const count = Number(params?.count ?? 0);
      text = message[`=${count}`] ?? message[plurals.select(count)] ?? message.other ?? "";
    }
    return text.replace(/\{([A-Za-z0-9_]+)\}/g, (whole, name: string) =>
      params && name in params ? String(params[name]) : whole
    );
  };
}

/** English, the default for helpers called without a language and for tests. */
export const englishT: Translate = translator("en");
