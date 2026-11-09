<!-- Explains the popup script. -->

# Popup (`browser-extension/src/popup`)

`popup.ts` drives `static/popup.html`. It shows whether the helper and the app are reachable
(with a "Check connection" button that starts the host) and which meetings are being recorded,
and binds the settings checkboxes to `chrome.storage.local`.
