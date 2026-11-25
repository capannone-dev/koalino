<!-- Explains the extension's source tree: one folder per extension context plus shared logic. -->

# Extension sources (`browser-extension/src`)

One folder per extension context, plus `lib/` for logic shared between them. esbuild bundles each
entry point into one script (`build.mjs`).

| Folder | Context | Entry point |
| --- | --- | --- |
| `page/` | A call page's own JavaScript world (`world: MAIN`, document start, or injected for Record this tab) | `hook.ts` → `hook.js` |
| `content/` | The isolated content-script world on call pages | `content.ts` → `content.js` |
| `background/` | The background (Chrome service worker, Firefox event page) | `background.ts` → `background.js` |
| `popup/` | The toolbar popup | `popup.ts` → `popup.js` |
| `lib/` | Pure logic, used by the others and covered by `tests/` | — |
| `locales/` | The text shown, one JSON file per language (#124) | — |
