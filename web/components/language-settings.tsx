// Renders the Settings card choosing the dashboard's language (Issue #124). The choice is kept in
// a cookie of this browser, so the server renders pages in it; "Same as the browser" removes it.

"use client";

import { useId, useState } from "react";
import { useRouter } from "next/navigation";
import { isLanguageCode, languageCookie, languages, type LanguageCode } from "@/lib/i18n";
import { useT } from "@/lib/i18n/client";

/** How long the choice is kept, in seconds (a year). */
const cookieMaxAgeSeconds = 365 * 24 * 60 * 60;

/**
 * Saves the language choice in this browser, or forgets it.
 *
 * Inputs: a language code, or `null` to follow the browser. Outputs: none.
 * Side effects: writes or expires the `koalino-language` cookie.
 */
export function storeLanguageChoice(language: LanguageCode | null): void {
  document.cookie =
    language === null
      ? `${languageCookie}=; path=/; max-age=0; samesite=lax`
      : `${languageCookie}=${language}; path=/; max-age=${cookieMaxAgeSeconds}; samesite=lax`;
}

/**
 * The language picker.
 *
 * Inputs: `initial`, the saved choice (`null` when the page follows the browser).
 * Outputs: a card with a select. Side effects: on a change, stores the choice and re-renders the
 * page on the server in the new language.
 */
export function LanguageSettings({ initial }: { initial: LanguageCode | null }) {
  const t = useT();
  const router = useRouter();
  const fieldId = useId();
  const hintId = useId();
  const [choice, setChoice] = useState<LanguageCode | null>(initial);

  function choose(value: string) {
    const next = isLanguageCode(value) ? value : null;
    setChoice(next);
    storeLanguageChoice(next);
    router.refresh();
  }

  return (
    <div className="card max-w-2xl space-y-1">
      <label htmlFor={fieldId} className="block font-semibold">{t("language")}</label>
      <p id={hintId} className="text-sm text-muted">{t("languageDescription")}</p>
      <select
        id={fieldId}
        aria-describedby={hintId}
        className="min-h-11 rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30"
        value={choice ?? ""}
        onChange={(event) => choose(event.target.value)}
      >
        <option value="">{t("followTheBrowser")}</option>
        {languages.map((language) => (
          <option key={language.code} value={language.code} lang={language.code}>
            {language.name}
          </option>
        ))}
      </select>
    </div>
  );
}
