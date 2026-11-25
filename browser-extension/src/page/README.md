<!-- Explains the page-world hook script. -->

# Page hook (`browser-extension/src/page`)

`hook.ts` runs in a call page's own JavaScript world, at document start, before the page's own
scripts: on Teams, on the other known meeting sites and, once the owner allows it, on every site
(#174). For "Record this tab" the background injects it mid-call instead. Elsewhere than Teams it
also reports the page's call activity (`report-call` → `call-activity`), mixes every audio track
the call receives as the remote side, and finds the shared screen among the page's videos
(`ShareTracker`), including the screen the owner shares (`getDisplayMedia` is wrapped). On Teams: Only there can it see the microphone stream Teams opens (`getUserMedia` is wrapped),
the `<audio>` element Teams plays the call through (in Chrome it is never added to the page, so
every element given a stream is noted through a wrapped `srcObject` setter, #160), and the responses of the page's own
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
