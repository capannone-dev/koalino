/**
 * Puts an extension page's fixed text in the browser's language (#124), for the popup and the
 * options page (#212): sets `lang` and `dir`, and the text of every element marked `data-i18n`.
 */
import { msg, textDirection, uiLanguage, type MessageKey } from "./i18n";

/** Side effects: Changes the page's text and its `lang` and `dir` attributes. */
export function translatePage(): void {
  document.documentElement.lang = uiLanguage();
  document.documentElement.dir = textDirection();
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n]")) {
    element.textContent = msg(element.dataset.i18n as MessageKey);
  }
}
