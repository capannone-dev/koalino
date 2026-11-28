<!-- Explains the options page script. -->

# Options page (`browser-extension/src/options`)

`options.ts` drives `static/options.html` (#212), the extension's settings page, opened from the popup's Settings
link or the browser's extension settings. It groups the switches as **All sites** (watch every website for calls,
which first asks the browser for access to all sites; keep shared slides; the per-site Always / Never choices and
Forget) and **Teams**. Each switch saves to `chrome.storage.local` as soon as it changes.
