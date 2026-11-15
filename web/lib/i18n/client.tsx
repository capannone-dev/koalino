"use client";

// Gives client components the page's language (Issue #124). The root layout resolves it on the
// server and passes it to `I18nProvider`, so server and browser render the same text. Without a
// provider, as in tests, everything is English.

import { createContext, useContext, useMemo, type ReactNode } from "react";
import { englishT, translator, type LanguageCode, type Translate } from "@/lib/i18n";

const LanguageContext = createContext<{ language: LanguageCode; t: Translate }>({ language: "en", t: englishT });

/**
 * Makes `language` the language of every client component below it.
 *
 * Inputs: the resolved language and the page. Outputs: the page. Side effects: none.
 */
export function I18nProvider({ language, children }: { language: LanguageCode; children: ReactNode }) {
  const value = useMemo(() => ({ language, t: translator(language) }), [language]);
  return <LanguageContext.Provider value={value}>{children}</LanguageContext.Provider>;
}

/**
 * The `Translate` for the page's language.
 *
 * Inputs: none. Outputs: a translator (English outside a provider). Side effects: none.
 */
export function useT(): Translate {
  return useContext(LanguageContext).t;
}

/**
 * The page's language code, for `Intl` formatting.
 *
 * Inputs: none. Outputs: a language code (`en` outside a provider). Side effects: none.
 */
export function useLanguage(): LanguageCode {
  return useContext(LanguageContext).language;
}
