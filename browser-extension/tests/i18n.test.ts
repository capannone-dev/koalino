/**
 * Specifies the extension's text in the browser's language (Issue #124): the `msg` helper, the
 * conversion of `src/locales/<lang>.json` into the browsers' `_locales` format, and that every
 * language has every message with the same placeholders.
 */
/// <reference types="vite/client" />
import { afterEach, describe, expect, it, vi } from "vitest";
import { msg, placeholderNames, textDirection } from "../src/lib/i18n";
import { chromeLocaleDirectory, chromeMessages } from "../src/lib/localeFormat";

/** Every language's source messages, by file path. */
const LOCALE_FILES = import.meta.glob<Record<string, string>>("../src/locales/*.json", {
  eager: true,
  import: "default",
});

/** One language's source messages. */
function readLocale(code: string): Record<string, string> {
  return LOCALE_FILES[`../src/locales/${code}.json`] ?? {};
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("msg", () => {
  it("falls back to English outside the extension and fills the placeholders", () => {
    expect(msg("recordingMeetings", { meetings: "Weekly sync" })).toBe("Recording Weekly sync");
    expect(msg("record")).toBe("Record");
  });

  it("passes the placeholder values in order to chrome.i18n", () => {
    const getMessage = vi.fn((key: string, values?: string[]) =>
      key === "transcriptFailed" ? `«${values?.join("|")}»` : "",
    );
    vi.stubGlobal("chrome", { i18n: { getMessage, getUILanguage: () => "it" } });
    expect(msg("transcriptFailed", { reason: "gone", title: "Sync" })).toBe("«Sync|gone»");
    expect(getMessage).toHaveBeenCalledWith("transcriptFailed", ["Sync", "gone"]);
  });

  it("uses English when the browser has no translation", () => {
    vi.stubGlobal("chrome", { i18n: { getMessage: () => "", getUILanguage: () => "it" } });
    expect(msg("stop")).toBe("Stop");
  });
});

describe("textDirection", () => {
  it("follows the language the messages are in", () => {
    expect(textDirection()).toBe("ltr");
    vi.stubGlobal("chrome", { i18n: { getMessage: (key: string) => (key === "textDirection" ? "rtl" : ""), getUILanguage: () => "en-US" } });
    expect(textDirection()).toBe("rtl");
  });
});

describe("chromeMessages", () => {
  it("turns {name} placeholders into numbered substitutions and escapes dollars", () => {
    expect(
      chromeMessages({ a: "Plain", b: "{title}: {reason} costs $5" }, { a: "Plain", b: "{title}: {reason}" }),
    ).toEqual({
      a: { message: "Plain" },
      b: {
        message: "$TITLE$: $REASON$ costs $$5",
        placeholders: { title: { content: "$1" }, reason: { content: "$2" } },
      },
    });
  });

  it("numbers placeholders in the English order even when a language reorders them", () => {
    expect(chromeMessages({ b: "{reason} — {title}" }, { b: "{title}: {reason}" })).toEqual({
      b: {
        message: "$REASON$ — $TITLE$",
        placeholders: { title: { content: "$1" }, reason: { content: "$2" } },
      },
    });
  });

  it("names the browsers' locale folders", () => {
    expect(chromeLocaleDirectory("pt")).toBe("pt_BR");
    expect(chromeLocaleDirectory("zh")).toBe("zh_CN");
    expect(chromeLocaleDirectory("it")).toBe("it");
  });
});

describe("the translations", () => {
  const english = readLocale("en");
  const codes = Object.keys(LOCALE_FILES).map((path) => path.replace(/^.*\/(\w+)\.json$/, "$1"));

  it("cover the ten app languages", () => {
    expect(codes.sort()).toEqual(["ar", "de", "en", "es", "fr", "it", "ja", "pt", "ru", "zh"]);
  });

  it.each(codes)("%s has every message with the English placeholders", (code: string) => {
    const messages = readLocale(code);
    expect(Object.keys(messages).sort()).toEqual(Object.keys(english).sort());
    for (const [key, text] of Object.entries(english)) {
      expect(placeholderNames(messages[key] ?? "").sort(), `${code}.${key}`).toEqual(placeholderNames(text).sort());
    }
  });
});
