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
| `sentMicrophone.ts` | Finds the owner's microphone as the audio track Teams sends on its call connection (#157), so a microphone Teams opened before the page hook ran is still recorded. `observeConnections` patches `RTCPeerConnection.prototype` (`getStats`, `addTrack`, `addTransceiver`, and the negotiation steps `setLocalDescription` and `setRemoteDescription` for receive-only calls, #174), which also reaches connections that already exist. A connection that sends audio without a live track means muted: no earlier `getUserMedia` track stands in. `claimHookGeneration` lets only the newest copy of the hook in a page act, because Firefox injects it again when the extension is installed or updated and the old copy stays. |
| `teamsDom.ts` | Reads Teams pages: call state, title, captions, transcript panel, share video, recap cards, and the recap view a meeting chat opens on (`findRecapView`). `withoutDoubledLabel` reads a Fluent tab's label, which an unselected tab holds twice. |
| `meetings.ts` | Records of recorded meetings, matching a recap to one (a recap view whose session times are not shown yet matches only recordings that saw Teams transcription, #159), picking the recap session nearest a recording (`closestSession`, #158), and when to retry a failed transcript fetch (`recapRetryDue`: a minute apart, three tries). A running recording notes `lastSeenMs` every 30 s (`heartbeatDue`), which stands in for the end when the page reloads during the call (`recordedEnd`). `resumableMeeting` picks the recording a reloaded tab continues: the one this tab ran, never ended, seen at most `RESUME_GAP_MS` (2 min) ago, with the same title when both are known. |
| `interruptions.ts` | `InterruptedRecordings` (background): when a recording tab's port closes mid-call, ends its meeting in Koalino after `RESUME_GAP_MS` unless a tab resumes it. Timers are injected for tests. |
| `callSignal.ts` | Tells a call on any site (#174): `callActivity` reads the page's call connections and the microphone and camera it opened; `looksLikeCall` starts a call once an open connection receives someone's audio and keeps it while the connection stays open; `CallDetector` starts a call after 3 s and ends it 8 s after; `remoteAudioTracks` lists every audio track the call receives, to mix them. |
| `screenShare.ts` | Finds the shared screen among a call's videos on any site (#174): `screenContentStats` and `looksLikeScreen` tell screen content (flat areas, few colours, text edges, little skin) from a camera; `ShareTracker` takes the screen the owner shares, else the largest video that looks like a screen and holds still, and keeps it while it stays screen content. |
| `sites.ts` | The known meeting sites (#174): Teams, Google Meet, Zoom, Jitsi, Webex, Whereby; their match patterns for the manifest, `siteName` and `siteKey` for per-site choices, and `meetingTitle` from a page's title. |
| `anySite.ts` | Brings the scripts beyond the known sites (#174): registers them for every https site while the owner wants it and allowed it (`syncAnySiteScripts`), and injects them into one tab for "Record this tab" where they are missing (`ensureTabScripts`). |
| `settings.ts` | Settings and their defaults, and the per-site Always / Never choices (`choiceForSite`, `withSiteChoice`, #174). |
| `storage.ts` | Settings and meeting records in `chrome.storage.local`. |
| `messages.ts` | The messages between hook, content script, background and native host, and the popup's progress line (`describeProgress`). |
| `i18n.ts` | `msg(key, params)`: a message in the browser's language through `chrome.i18n`, else English; `textDirection` and `uiLanguage` for `dir` and `lang`, read from the messages so they match the text (#124). |
| `localeFormat.ts` | Turns `src/locales/<lang>.json` into the browsers' `_locales/<dir>/messages.json` (`$NAME$` placeholders numbered in English order); `build.mjs` imports it. |
| `siteAccess.ts` | Checks and asks for access to the meeting sites (the manifest's host permissions), which Firefox does not grant a hand-loaded Manifest V3 extension and Chrome lets the owner withhold (they include SharePoint, for the recap transcript frame), and `embeddedInTeams`, which tells a frame Teams opened from any other SharePoint page. |

`popupStatus.ts` (#212) words the popup's connection line and its tone; `pageText.ts` puts the popup's and the
options page's fixed text in the browser's language.
