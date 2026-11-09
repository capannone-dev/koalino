<!-- Explains the page-world hook script. -->

# Page hook (`browser-extension/src/page`)

`hook.ts` runs in the Teams page's own JavaScript world, at document start, before Teams' own
scripts. Only there can it see the microphone stream Teams opens (`getUserMedia` is wrapped),
the `<audio>` elements Teams plays the call through, and the responses of the page's own
transcript requests (`fetch` is wrapped and the response is copied, never re-requested). It
taps audio only between the content script's `start-audio` and `stop-audio` commands, and posts
16 kHz PCM and transcript files to the content script with `window.postMessage`.
