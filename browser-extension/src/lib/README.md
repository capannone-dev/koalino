<!-- Explains the extension's shared, browser-independent logic modules. -->

# Shared logic (`browser-extension/src/lib`)

Plain TypeScript with no extension APIs, except `storage.ts`, so it runs in every context and in
the Vitest specs under `tests/`.

| File | Purpose |
| --- | --- |
| `pcm.ts` | Float audio to 16 kHz signed 16-bit PCM, 10 s chunks stamped with meeting time, silence check, base64. |
| `vtt.ts` | WebVTT writer (timestamps, voice-tagged cues). |
| `captions.ts` | Follows growing live captions and the transcript panel into cues. |
| `slides.ts` | Finds the slide inside a shared frame (port of the #102 crop prototype) and keeps one frame per settled slide: at least 8 of 64×36 cells must change, so slides sharing a layout count and the pointer does not. |
| `teamsDom.ts` | Reads Teams pages: call state, title, captions, transcript panel, share video, recap cards. |
| `meetings.ts` | Records of recorded meetings, and matching a recap card to one. |
| `settings.ts` | Settings and their defaults. |
| `storage.ts` | Settings and meeting records in `chrome.storage.local`. |
| `messages.ts` | The messages between hook, content script, background and native host, and the popup's progress line (`describeProgress`). |
| `siteAccess.ts` | Checks and asks for access to the Teams sites, which Firefox does not grant a hand-loaded Manifest V3 extension and Chrome lets the owner withhold. |
