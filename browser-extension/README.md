<!-- Explains the Note Taker browser extension: what it records, how it reaches the app, and how to build, install and test it. -->

# Note Taker browser extension (`browser-extension/`)

Records Microsoft Teams meetings joined in the browser (Firefox, Chrome, Edge, Chromium) and
sends them to the Note Taker app on the same computer (Issue #102). It works for work or school
accounts (`teams.cloud.microsoft`, `teams.microsoft.com`) and for personal accounts
(`teams.live.com`, `teams.microsoft.com/light-meetings`).

## What it does

When a call starts it asks **Record this meeting with Note Taker?** (Record, Always for
Teams, Not now). While recording, an indicator with a Stop button stays on the page. It sends:

| What | How | Becomes in Note Taker |
| --- | --- | --- |
| Your microphone | the audio track Teams opened, tapped in the page | your segments of the transcript |
| Everyone else | the one mixed track Teams plays, tapped in the page | segments whose voices are matched against your voice gallery (`MIXED_REMOTE`) |
| Live captions | turned on for you (only you see them) and read as they grow | a `teams-captions` WebVTT attachment with names and times |
| Teams transcript panel | read while Teams transcription runs | a `teams-live-transcript` attachment |
| Shared slides | a frame of the screen share once it changes and holds still, cropped to the slide | slide attachments at their point in the meeting |
| The Teams transcript | fetched from the recap Teams posts in the meeting chat, whoever started transcription | a `teams-transcript` WebVTT attachment of the same meeting |

Optional, off by default: **Start Teams transcription when I record** (More → Record and
transcribe → Start transcription). Everyone in the meeting sees it, and Microsoft stores the
transcript.

Limits found in the #102 probes:

- Teams publishes the transcript only to the organizer's organisation. As a guest, the
  extension keeps the captions and the live transcript panel.
- Debian's Firefox ESR turns OpenH264 off (`/etc/firefox-esr/firefox-esr.js`), so Teams shows no
  screen share and no slides can be kept. Turn it back on in the profile's `user.js` (see below).

## How it reaches the app

```text
Teams page ── hook.js (page world: getUserMedia, <audio>, fetch) ─postMessage─▶ content.js
content.js ─runtime port─▶ background.js ─connectNative─▶ browser-host ─WebSocket 127.0.0.1─▶ daemon (Teams bridge)
```

Browsers cannot open the bridge's WebSocket themselves (it refuses an `Origin` header), so the
native messaging host `browser-host` (`core/bin/browser_host.rs`,
`core/adapters/network/browser_host/`) carries the messages. The daemon must run with
`NOTE_TAKER_TEAMS_BRIDGE_ENABLED=true`.

## Layout

| Path | Purpose |
| --- | --- |
| `src/page/` | `hook.ts`, the page-world script (audio taps, transcript copy, share frames for slides). |
| `src/content/` | The content script: call detection, consent, recording, Teams automation, overlay. |
| `src/background/` | The background: the native host connection and the popup's state. |
| `src/popup/` | The popup: connection status and settings. |
| `src/lib/` | Browser-independent logic: PCM, WebVTT, captions, slides, Teams DOM readers, meeting records, settings, messages. |
| `static/` | `popup.html` and icons. |
| `tests/` | Vitest specs (jsdom). |
| `build.mjs` | Builds `dist/firefox` and `dist/chromium`, with each browser's manifest. |

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
2. Start the daemon with `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=true`.
3. Load the extension:
   - Chrome or Edge: Extensions → Developer mode → Load unpacked → `dist/chromium`. The ID must
     be `dgmaacilnokmfecfdcajdjadeboemnpd`.
   - Firefox: `about:debugging` → This Firefox → Load Temporary Add-on → `dist/firefox/manifest.json`.
     Unsigned add-ons stay installed only in ESR, Developer Edition or Nightly with
     `xpinstall.signatures.required` set to false.
4. Firefox does not grant a hand-loaded extension its site access. The extension then opens its
   popup in a tab: press **Allow on Teams** and accept, then reload any open Teams tab. The popup
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
only to the local Note Taker app over the loopback interface. The extension never calls
Microsoft's APIs and never uses the page's tokens: it copies only the transcript file the Teams
page itself loads. It keeps in browser storage only its settings and, for two weeks, the title
and times of the meetings it recorded, so it can match their Teams transcripts. Nothing of a
meeting is logged.
