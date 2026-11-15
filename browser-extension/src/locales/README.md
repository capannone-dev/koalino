<!-- Explains the extension's translation sources. -->

# Translations (`browser-extension/src/locales`)

The extension's text, one flat JSON file per language: `en`, `it`, `es`, `fr`, `de`, `pt`
(Brazilian), `zh` (Simplified), `ja`, `ru`, `ar` (right to left), the same ten languages as the
app and the dashboard (#124).

- Keys are message names (`[A-Za-z0-9_]`); `{name}` marks a value filled in at run time.
  English defines every key, and `tests/i18n.test.ts` checks that every other file has the same
  keys and placeholders.
- `languageTag` and `textDirection` give the popup's `lang` and `dir`.
- `node build.mjs` writes them into `dist/<browser>/_locales/<lang>/messages.json` (`pt` → `pt_BR`,
  `zh` → `zh_CN`) through `src/lib/localeFormat.ts`; `msg` in `src/lib/i18n.ts` reads them.
- The browser chooses the language from its own UI language, with English as the fallback;
  extensions get no per-extension override.
- `extName` and `extDescription` name the extension in the browser's add-on list.

To add a message: add it to `en.json`, then to every other file, run `npx vitest run`.
Translations are machine-made and await a native review.
