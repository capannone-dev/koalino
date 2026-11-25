<!-- Explains the extension's shared, browser-independent logic modules. -->

# Shared logic (`browser-extension/src/lib`)

Plain TypeScript with no extension APIs, except `storage.ts` (and `i18n.ts`, which falls back to
English without them), so it runs in every context and in
the Vitest specs under `tests/`.

| File | Purpose |
| --- | --- |
| `pcm.ts` | Float audio to 16 kHz signed 16-bit PCM, 10 s chunks stamped with meeting time, silence check, base64. |
| `vtt.ts` | WebVTT writer (timestamps, voice-tagged cues). |
| `captions.ts` | Follows growing live captions and the transcript panel into cues. |
| `slides.ts` | Finds the slide inside a shared frame (port of the #102 crop prototype) and keeps one frame per settled slide: at least 8 of 64×36 cells must change, so slides sharing a layout count and the pointer does not. |
| `remoteAudio.ts` | Finds the remote call audio (#160). `observePlayedStreams` wraps `HTMLMediaElement.prototype.srcObject` so every element Teams gives a stream is reported, including Chrome's muted `<audio>` element that is never added to the page (Teams plays its stream through Web Audio there). `remoteCallStream` picks the stream with a live track that is not the owner's microphone. |
| `sentMicrophone.ts` | Finds the owner's microphone as the audio track Teams sends on its call connection (#157), so a microphone Teams opened before the page hook ran is still recorded. `observeConnections` patches `RTCPeerConnection.prototype` (`getStats`, `addTrack`, `addTransceiver`), which also reaches connections that already exist. A connection that sends audio without a live track means muted: no earlier `getUserMedia` track stands in. `claimHookGeneration` lets only the newest copy of the hook in a page act, because Firefox injects it again when the extension is installed or updated and the old copy stays. |
| `teamsDom.ts` | Reads Teams pages: call state, title, captions, transcript panel, share video, recap cards, and the recap view a meeting chat opens on (`findRecapView`). `withoutDoubledLabel` reads a Fluent tab's label, which an unselected tab holds twice. |
| `meetings.ts` | Records of recorded meetings, matching a recap to one (a recap view whose session times are not shown yet matches only recordings that saw Teams transcription, #159), picking the recap session nearest a recording (`closestSession`, #158), and when to retry a failed transcript fetch (`recapRetryDue`: a minute apart, three tries). A running recording notes `lastSeenMs` every 30 s (`heartbeatDue`), which stands in for the end when the page reloads during the call (`recordedEnd`). `resumableMeeting` picks the recording a reloaded tab continues: the one this tab ran, never ended, seen at most `RESUME_GAP_MS` (2 min) ago, with the same title when both are known. |
| `interruptions.ts` | `InterruptedRecordings` (background): when a recording tab's port closes mid-call, ends its meeting in Koalino after `RESUME_GAP_MS` unless a tab resumes it. Timers are injected for tests. |
| `settings.ts` | Settings and their defaults. |
| `storage.ts` | Settings and meeting records in `chrome.storage.local`. |
| `messages.ts` | The messages between hook, content script, background and native host, and the popup's progress line (`describeProgress`). |
| `i18n.ts` | `msg(key, params)`: a message in the browser's language through `chrome.i18n`, else English; `textDirection` and `uiLanguage` for `dir` and `lang`, read from the messages so they match the text (#124). |
| `localeFormat.ts` | Turns `src/locales/<lang>.json` into the browsers' `_locales/<dir>/messages.json` (`$NAME$` placeholders numbered in English order); `build.mjs` imports it. |
| `siteAccess.ts` | Checks and asks for access to the Teams sites, which Firefox does not grant a hand-loaded Manifest V3 extension and Chrome lets the owner withhold (they include SharePoint, for the recap transcript frame), and `embeddedInTeams`, which tells a frame Teams opened from any other SharePoint page. |
