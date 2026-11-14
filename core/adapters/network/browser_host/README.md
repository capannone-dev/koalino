<!-- Explains the browser extension's native messaging host: what it does, its files, and its security. -->

# Browser extension host (`core/adapters/network/browser_host`)

The Koalino browser extension (`browser-extension/`, Issue #102) captures an online meeting in
the browser. Browsers cannot open the Teams bridge's WebSocket themselves: the bridge refuses
every handshake with an `Origin` header, which browsers always send. So the extension talks to a
**native messaging host**, the `browser-host` program (`core/bin/browser_host.rs`). The browser
starts it, connects it to the extension over stdin and stdout, and stops it when the extension
disconnects. The host converts each JSON message into a Teams bridge frame and sends it to the
daemon on `ws://127.0.0.1:<port>`, without an `Origin` header.

```text
extension (background) ──connectNative("com.note_taker.browser_bridge")──▶ browser-host
   JSON: meeting, audio, slide, document, ended       stdin ──▶ extension_message ──▶ bridge_client ──WS──▶ daemon
   ◀── {"type":"status","bridge":…} / {"type":"rejected"}   stdout
```

| File | Purpose |
| --- | --- |
| `native_messaging.rs` | The framing: a 32-bit native-endian length, then UTF-8 JSON. Incoming messages are capped at 16 MiB (checked before reading), outgoing at the browsers' 1 MB. |
| `extension_message.rs` | The extension's message schema (table in the file header) and its conversion into `TeamsBridgeMessage` frames. Microphone audio is a `PARTICIPANT` track of the owner; remote audio is the `MIXED_REMOTE` track `browser-remote-mix`. |
| `bridge_client.rs` | The WebSocket client: optional authentication, ordered delivery, a queue bounded by audio length (audio dropped oldest first; meetings, slides, documents and meeting ends always kept), and reconnection every 2 s while the daemon is down. |
| `host_session.rs` | One session: a blocking stdin reader, the client, and a stdout writer for status and rejections. |
| `host_install.rs` | The host manifests and where each browser looks for them: files under `~/.mozilla` and `~/.config/<browser>` on Linux; files next to the host plus `HKCU\Software\…\NativeMessagingHosts` keys on Windows. On Windows, paths are written without `canonicalize`'s `\\?\` prefix: Chrome finds neither a manifest nor a host through it. |
| `*_tests.rs` | TDD specs. The client and session tests run against the real `TeamsWebSocketServer`. |

## Security

- Only the Koalino extension may start the host: the manifests allow only the Firefox add-on
  `browser-bridge@note-taker.local` and the Chromium origin
  `chrome-extension://dgmaacilnokmfecfdcajdjadeboemnpd/`. That ID is fixed by the public `key`
  in the extension's Chromium manifest.
- The host connects to the loopback interface only.
- When a bridge secret exists, the host authenticates with it. `browser-host install` creates
  a random secret once in the keychain (`note-taker` / `teams-bridge-secret`), and the daemon
  reads it there when `NOTE_TAKER_TEAMS_BRIDGE_SECRET` is not set. Without a keychain (for
  example a desktop with no Secret Service), there is no secret and the bridge accepts any local
  program, as before.
- Logs go to stderr and contain status and counts only: no audio, slides, transcript text,
  names or the secret.
