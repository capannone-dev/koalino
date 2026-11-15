<!-- Explains the static files copied into each build. -->

# Static files (`browser-extension/static`)

Copied unchanged into `dist/firefox` and `dist/chromium`: `popup.html` and the toolbar icons
(`icon-48.png`, `icon-128.png`). The popup's fixed text is English in the file; elements marked
`data-i18n` get the browser's language from `popup.ts` (#124).
