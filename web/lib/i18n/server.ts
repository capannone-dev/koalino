// Gives server pages the language of the request (Issue #124): the owner's cookie, else the
// browser's `Accept-Language`. Server-only: it reads the request through `next/headers`.

import { cookies, headers } from "next/headers";
import { languageCookie, matchLanguage, translator, type LanguageCode, type Translate } from "@/lib/i18n";

/**
 * The language this request is shown in.
 *
 * Inputs: none (the current request). Outputs: a language code. Side effects: reads the request's
 * cookies and headers, which makes the page render per request.
 */
export async function requestLanguage(): Promise<LanguageCode> {
  const [cookieStore, headerList] = await Promise.all([cookies(), headers()]);
  return matchLanguage(cookieStore.get(languageCookie)?.value, headerList.get("accept-language"));
}

/**
 * The `Translate` for this request, for server components.
 *
 * Inputs: none. Outputs: the request language's translator. Side effects: as `requestLanguage`.
 */
export async function getT(): Promise<Translate> {
  return translator(await requestLanguage());
}
