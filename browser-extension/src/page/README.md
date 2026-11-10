<!-- Explains the page-world hook script. -->

# Page hook (`browser-extension/src/page`)

`hook.ts` runs in the Teams page's own JavaScript world, at document start, before Teams' own
scripts. Only there can it see the microphone stream Teams opens (`getUserMedia` is wrapped),
the `<audio>` elements Teams plays the call through, and the responses of the page's own
transcript requests (`fetch` is wrapped and the response is copied, never re-requested). It
taps audio only between the content script's `start-audio` and `stop-audio` commands, and posts
16 kHz PCM and transcript files to the content script with `window.postMessage`.

It also reads the screen share for slide capture. On `analyse-share` it draws the shared screen's
video on a canvas and posts only the frame's 64×36 signature. On `encode-slide` it crops that
frame to the slide and posts a base64 JPEG. Pixels are read here, not in the content script.
In Firefox, a content script reading a page video's pixels crawls through per-byte wrappers: it
froze Teams in the #102 live test. Awaiting the page's own promises is denied instead.

The hook also runs in SharePoint frames. It acts only in a frame a Teams page opened (the recap's
transcript frame), and there it only copies transcript downloads (`src/recap/`). On any other
SharePoint page it does nothing.
