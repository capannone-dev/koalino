<!-- Explains the popup script. -->

# Popup (`browser-extension/src/popup`)

`popup.ts` drives `static/popup.html`. It shows whether the helper and the app are reachable
(with a "Check connection" button that starts the host) and which meetings are being recorded,
and binds the settings checkboxes to `chrome.storage.local`. It shows what happened to the last Teams transcript fetch (sent, already sent, no matching
recording, or the step that failed). While recording it shows what was sent
(audio chunks, slides, captions) and the last problem. When the extension may not run on the meeting
sites, it shows an "Allow on meeting sites" button that asks the browser for that access.

Since #174 it has **Record this tab** (Stop on a tab that records; through the background), and
**Watch every website for calls**, which asks the browser for access to every https site before
it turns on. It lists the per-site Always / Never choices with a button to forget them, and groups
the Teams-only settings under their own heading.

Since #212 the popup is only status and action: it checks the connection itself when opened (a "Check again"
button appears only when something is wrong), has one main button (Record this tab, or Stop), shows the access
banner only when access is missing, and links to the options page (`src/options/`), where the settings now live.
