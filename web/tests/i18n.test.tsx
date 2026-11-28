// Verifies the dashboard's translations (Issue #124): every language has every message with the
// same placeholders, the language is picked from the owner's choice or the browser, plurals and
// placeholders are filled in, and the Settings picker stores the choice and re-renders.

import { fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, test, vi } from "vitest";
import { LanguageSettings } from "@/components/language-settings";
import { PrimaryNav } from "@/components/primary-nav";
import { dictionaries, languages, matchLanguage, textDirection, translator, type Message } from "@/lib/i18n";
import { I18nProvider } from "@/lib/i18n/client";

const refresh = vi.fn();
vi.mock("next/navigation", () => ({ useRouter: () => ({ push: vi.fn(), refresh }), usePathname: () => "/" }));

afterEach(() => {
  document.cookie = "koalino-language=; path=/; max-age=0";
  refresh.mockReset();
});

/** The `{name}` placeholders of a message, across its plural forms. */
function placeholders(message: Message): string[] {
  const texts = typeof message === "string" ? [message] : Object.values(message);
  return [...new Set(texts.flatMap((text) => [...text.matchAll(/\{([A-Za-z0-9_]+)\}/g)].map((match) => match[1])))].sort();
}

test("every language has every English message, with the same placeholders and an other form", () => {
  const english = dictionaries.en;
  expect(Object.keys(dictionaries).sort()).toEqual(languages.map((language) => language.code).sort());
  for (const [code, dictionary] of Object.entries(dictionaries)) {
    expect(Object.keys(dictionary).sort(), code).toEqual(Object.keys(english).sort());
    for (const [key, message] of Object.entries(english)) {
      const translated = dictionary[key];
      expect(placeholders(translated), `${code} ${key}`).toEqual(placeholders(message));
      expect(typeof translated === typeof message, `${code} ${key}`).toBe(true);
      if (typeof translated !== "string") expect(translated.other, `${code} ${key}`).toBeTruthy();
    }
  }
});

test("the owner's choice wins, then the browser's languages in order of preference, then English", () => {
  expect(matchLanguage("ja", "it-IT,it;q=0.9")).toBe("ja");
  expect(matchLanguage(null, "nl-NL,de;q=0.8,it;q=0.9")).toBe("it");
  expect(matchLanguage("xx", "zh-TW")).toBe("zh");
  expect(matchLanguage(undefined, "nl,sv;q=0.5")).toBe("en");
  expect(matchLanguage(null, null)).toBe("en");
  expect(textDirection("ar")).toBe("rtl");
  expect(textDirection("fr")).toBe("ltr");
});

test("messages fill in placeholders and pick the plural form", () => {
  const english = translator("en");
  expect(english("dayCount", { count: 1 })).toBe("1 day");
  expect(english("dayCount", { count: 7 })).toBe("7 days");
  expect(english("movedTo", { project: "Budget" })).toBe("Moved to Budget");
  expect(translator("ru")("dayCount", { count: 5 })).toBe("5 дней");
  expect(translator("ru")("dayCount", { count: 2 })).toBe("2 дня");
});

test("components below the provider speak its language", () => {
  render(
    <I18nProvider language="it">
      <PrimaryNav />
    </I18nProvider>
  );

  expect(screen.getByRole("link", { name: "Impostazioni" })).toHaveAttribute("href", "/settings");
});

test("choosing a language stores it in this browser and re-renders; Same as the browser forgets it", () => {
  render(<LanguageSettings initial={null} />);
  const select = screen.getByLabelText("Language");

  fireEvent.change(select, { target: { value: "de" } });
  expect(document.cookie).toContain("koalino-language=de");
  expect(refresh).toHaveBeenCalledTimes(1);

  fireEvent.change(select, { target: { value: "" } });
  expect(document.cookie).not.toContain("koalino-language=de");
  expect(refresh).toHaveBeenCalledTimes(2);
});
