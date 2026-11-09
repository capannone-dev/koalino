<!-- Explains the background script. -->

# Background (`browser-extension/src/background`)

`background.ts` owns the connection to the native messaging host
`com.note_taker.browser_bridge`. It starts the host on the first message, forwards the tabs'
messages in order, passes the host's status (`connected` / `unavailable`) to every tab, and
answers the popup's state requests. When the browser cannot start the host, it reports it as
`missing` with the browser's reason.
