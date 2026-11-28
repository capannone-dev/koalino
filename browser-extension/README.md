<!-- Explains the Koalino browser extension: what it records, how it reaches the app, and how to build, install and test it. -->

# Koalino browser extension (`browser-extension/`)

Records calls joined in the browser (Firefox, Chrome, Edge, Chromium) and sends them to the
Koalino app on the same computer (Issue #102). It started with Microsoft Teams, for work or
school accounts (`teams.cloud.microsoft`, `teams.microsoft.com`) and personal accounts
(`teams.live.com`, `teams.microsoft.com/light-meetings`). Since #174 it records calls on any site.

## Any call, on any site (#174)

| Where | How it starts |
| --- | --- |
| Teams, Google Meet, Zoom (web client), Jitsi Meet (`meet.jit.si`, `8x8.vc`), Webex, Whereby | It notices the call and asks **Looks like a call on Google Meet. Record it with Koalino?** (Record, Always here, Not now, Never here). Listed in `src/lib/sites.ts`. |
| Any other https site | **Watch every website for calls** in the popup (off by default) asks the browser for access to all sites, then watches them the same way. |
| Any tab, at any time | **Record this tab** in the popup. The browser grants that one tab when the popup opens (`activeTab`), so no access to other sites is needed. The recording runs until you press Stop, or until the call it saw ends. |

A call starts when an open WebRTC connection receives someone's audio for 3 seconds; it goes on
while that connection stays open and is over 8 seconds after it closes (`src/lib/callSignal.ts`).
The microphone or camera alone is no call: dictation sites use them too, and Google Meet's
pre-join screen already sends both on a connected connection before anyone is heard. Elsewhere
than Teams it records:

| What | How |
| --- | --- |
| Your microphone | the audio track the page sends on its call connection, as on Teams |
| Everyone else | every audio track the call receives, mixed (Google Meet plays the three loudest speakers as separate tracks) |
| Shared slides | the screen you share (`getDisplayMedia`), else the video that looks like a still screen: large areas of one colour, a few colours covering most of it, sharp text edges, little skin, and the same frame for seconds (`src/lib/screenShare.ts`). Of several, the largest. Then cropped and kept once per slide, as on Teams. |

The meeting is titled from the page (`Weekly sync | Jitsi Meet` → `Weekly sync`; a Google Meet
code as `Google Meet abc-defg-hij`), else **Call on <site>**. Captions, the transcript panel and
the Teams transcript remain Teams-only. While the extension records, the Koalino app does not
also offer to record the call, and withdraws an offer it already showed.

Services that play call audio without WebRTC (decoded in the page and played through Web Audio)
are not covered yet; Zoom's web client, Meet, Jitsi, Webex and Whereby use WebRTC.

## On Teams

When a call starts it asks **Record this meeting with Koalino?** (Record, Always for
Teams, Not now). While recording, an indicator with a Stop button stays on the page. It sends:

| What | How | Becomes in Koalino |
| --- | --- | --- |
| Your microphone | the audio track Teams sends in the call (found even when Teams opened it before the extension loaded, #157), tapped in the page | your segments of the transcript |
| Everyone else | the one mixed track Teams plays, tapped in the page | segments whose voices are matched against your voice gallery (`MIXED_REMOTE`) |
| Live captions | turned on for you (only you see them) and read as they grow | a `teams-captions` WebVTT attachment with names and times |
| Teams transcript panel | read while Teams transcription runs | a `teams-live-transcript` attachment |
| Shared slides | a frame of the screen share once it changes and holds still, cropped to the slide; only the shared-screen stream is read, never a camera (#137) | slide attachments at their point in the meeting |
| The Teams transcript | fetched from the recap in the meeting chat, whoever started transcription: the extension opens the Transcript tab and uses the recap's Download → .vtt (which also leaves a copy in Downloads) | a `teams-transcript` WebVTT attachment of the same meeting |

Optional, off by default: **Start Teams transcription when I record** (More → Record and
transcribe → Start transcription). Everyone in the meeting sees it, and Microsoft stores the
transcript.

Optional, off by default: **Set Teams' spoken language to Koalino's** (#141). Koalino names its
default transcription language when the host connects (nothing for Auto); about 20 s into a
recording, before transcription would start, the extension opens the live captions' settings
(gear → Meeting spoken language, where Teams keeps it since November 2026) and picks that
language, by the locale in the item's `data-tid` (`it-it`) or by its name. Older Teams versions
have it under More → Language and speech → Change spoken language, which the extension uses when
captions are not shown (it picks the language there and presses Update). A language Teams already uses is left alone; one Teams does not offer is
cancelled and shown as a problem in the popup. Teams applies it to the whole meeting.

**A reload stays one meeting.** If the Teams page reloads (or the owner rejoins) in the same tab
within two minutes, recording continues the same Koalino meeting, without asking again: the
owner already chose to record that call. The captions are kept in two files, before and after.
If the tab does not come back, the background ends the meeting in Koalino two minutes after the
page went away.

Limits found in the #102 probes:

- Teams publishes the transcript only to the organizer's organisation. As a guest, the
  extension keeps the captions and the live transcript panel.
- Debian's Firefox ESR turns OpenH264 off (`/etc/firefox-esr/firefox-esr.js`), so Teams shows no
  screen share and no slides can be kept. Turn it back on in the profile's `user.js` (see below).

## How it reaches the app

```text
call page ── hook.js (page world: call connections, getUserMedia, getDisplayMedia, <audio>/<video>, fetch on Teams) ─postMessage─▶ content.js
content.js ─runtime port─▶ background.js ─connectNative─▶ browser-host ─WebSocket 127.0.0.1─▶ daemon (Teams bridge)
recap frame (SharePoint, opened by Teams): hook.js (copies the .vtt download) ─▶ recap-frame.js ─port─▶ background.js ─▶ content.js
```

After the meeting, the recap's transcript is in a SharePoint frame, so the extension also asks
for access to `*.sharepoint.com`. It acts there only in frames a Teams page opened.

Browsers cannot open the bridge's WebSocket themselves (it refuses an `Origin` header), so the
native messaging host `browser-host` (`core/bin/browser_host.rs`,
`core/adapters/network/browser_host/`) carries the messages. On a desktop the Koalino app itself
receives them (#123): its bridge is on unless `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=false`. On a server,
start the daemon with `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=true`.

## Layout

| Path | Purpose |
| --- | --- |
| `src/page/` | `hook.ts`, the page-world script (call activity, audio taps, transcript copy, share frames for slides). |
| `src/content/` | The content script: call detection, consent, recording, Teams automation, overlay. |
| `src/recap/` | The recap frame script: downloads the Teams transcript inside the recap's SharePoint frame. |
| `src/background/` | The background: the native host connection, the popup's state, "Record this tab", the every-site scripts and the REC badge. |
| `src/popup/` | The popup: connection status, recording progress, Record this tab, the Teams transcript fetch's status, and settings. |
| `src/lib/` | Browser-independent logic: PCM, WebVTT, captions, slides, call and screen-share detection, meeting sites, Teams DOM readers, meeting records, settings, messages, translations. |
| `src/locales/` | The extension's text in ten languages (#124); the build turns it into each browser's `_locales`. |
| `static/` | `popup.html` and icons. |
| `tests/` | Vitest specs (jsdom). |
| `sign-firefox.sh` | Signs `dist/firefox` as an unlisted add-on on addons.mozilla.org into `dist/signed`, with the owner's API key from `~/.config/koalino/amo.env` (passed through web-ext's environment variables, never printed). Each signing needs a new `version` in `build.mjs`. With `--listed` it submits the version to the public AMO catalogue instead (#127), with the listing in `amo-listing.json` and a source zip for Mozilla's reviewers, and does not wait for the review. |
| `amo-listing.json` | The public AMO listing (summary, description, category, and the licence as a custom one: AMO offers only "GPL-3.0-only", not "or later") and the notes for Mozilla's reviewers, sent by `sign-firefox.sh --listed`. |
| `build.mjs` | Builds `dist/firefox` and `dist/chromium`, with each browser's manifest and `_locales`. |

Mozilla reviews each listed version by hand, so the add-ons site can lag behind for days. The
release's `koalino-browser-extension-firefox-VERSION.xpi` is therefore always the newest code,
signed as an **unlisted** version (`./sign-firefox.sh`, signed within minutes): raise the
version, sign it, and attach it to the release with the Chromium zip built from the same code
(and update `SHA256SUMS.txt`). Then submit the same code with `--listed` under the next version.

## Build, test, install

```sh
cd browser-extension
npm install
npx vitest run        # unit tests
npx tsc --noEmit      # type check
node build.mjs        # dist/firefox and dist/chromium
```

1. Build and register the host once per user: `cargo build --release --bin browser-host`, then
   `target/release/browser-host install`. This writes the manifests for Firefox, Chrome,
   Chromium and Edge (on Windows also the `HKCU` registry keys), and creates the bridge secret in
   the keychain when there is one.
2. Start the Koalino desktop app (or, on a server, the daemon with
   `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=true`).
3. Load the extension:
   - Chrome or Edge: Extensions → Developer mode → Load unpacked → `dist/chromium`. The ID must
     be `dgmaacilnokmfecfdcajdjadeboemnpd`.
   - Firefox (140 or newer): open the signed `.xpi` from the release (or from `dist/signed`) in
     Firefox and press **Add**. For development, `about:debugging` → This Firefox → Load Temporary
     Add-on → `dist/firefox/manifest.json` loads the unsigned build until Firefox restarts.
4. Firefox does not grant a hand-loaded extension its site access. The extension then opens its
   popup in a tab: press **Allow on meeting sites** and accept, then reload any open meeting tab. The popup
   shows the same button later if the access is removed.
5. Debian's Firefox ESR only: add to the profile's `user.js` and restart Firefox, so screen shares
   (and slides) show:

   ```js
   user_pref("media.gmp-gmpopenh264.enabled", true);
   user_pref("media.gmp-gmpopenh264.visible", true);
   user_pref("media.peerconnection.video.h264_enabled", true);
   ```

   Firefox then downloads the OpenH264 plugin from Cisco on its own.

## Privacy

Nothing is recorded until you agree, and the indicator stays visible while recording. Data goes
only to the local Koalino app over the loopback interface. The extension never calls
Microsoft's APIs and never uses the page's tokens: it copies only the transcript file the Teams
page itself loads (after the meeting, by pressing the recap's own Download → .vtt). On SharePoint it
acts only in the recap frame Teams opens. It keeps in browser storage only its settings and, for two weeks, the title
and times of the meetings it recorded, so it can match their Teams transcripts. Nothing of a
meeting is logged.
