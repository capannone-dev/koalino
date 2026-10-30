<!-- Provides the project entry point: what Note Taker is, how to build and run it, and how to configure it. -->

# Note Taker

Note Taker is a decentralized, offline-first meeting-notes application. It records meetings,
transcribes them with Whisper (on-device with whisper.cpp, or with the OpenAI API by opt-in),
groups transcript segments into known and unknown voices, and keeps notes in a CRDT-backed SQLite
store. Voice embeddings are encrypted at rest. The Rust core uses hexagonal architecture
(ports and adapters), so you can swap the AI, network, storage, and export implementations
without touching domain logic.

> **Status: pre-1.0.** Recording, transcription, the meetings workflow, export, telemetry, and
> structured logging work today, as do P2P sync between devices (#43), microphone capture (#45),
> and on-device speaker diarization (#44), which is opt-in: a `diarization` build with exported
> models. The apps do not ship those models yet. See
> [AUDIT_V1.md](AUDIT_V1.md) for the full compliance matrix and readiness assessment.

## Components

| Directory | What it is | Stack |
| --- | --- | --- |
| [`core/`](core/README.md) | Domain, application services, ports and adapters, plus the `cli`, `daemon`, and FFI (`libnote_taker_core`) entry points | Rust |
| [`ui/`](ui/README.md) | Desktop (Linux/Windows) and Android app, which calls the core through its C ABI | Flutter |
| [`web/`](web/README.md) | Dashboard for the headless daemon (meetings, voice gallery, telemetry) | Next.js |
| [`teams-bot/`](teams-bot/README.md) | Microsoft Teams media bot that streams meeting audio and slides to the daemon | C# / .NET |
| [`server/`](server/README.md) | Headless Linux deployment notes (systemd, background recording) | — |

## Building

Prerequisites: a stable Rust toolchain. Add the Flutter SDK for `ui/`, Node.js for `web/`, and
the .NET SDK for `teams-bot/`.

```sh
make rust-lint        # cargo fmt --check + cargo clippy -D warnings
make rust-test        # unit tests (AI models are mocked)
make build-server     # target/debug/daemon
cargo build --bin cli # target/debug/cli
cargo build --lib     # target/debug/libnote_taker_core.so, loaded by the Flutter app
make flutter-lint && make flutter-test
make build-desktop    # Flutter Linux desktop bundle
```

The default build has no native model dependencies. Opt into on-device inference with Cargo
features:

```sh
cargo build --features local-ai,vad   # whisper.cpp (needs cmake and a C++ toolchain) + WebRTC VAD
cargo build --features diarization    # speaker diarization; downloads ONNX Runtime at build time
```

Run with `NOTE_TAKER_WHISPER_MODEL` set to a GGML model, such as whisper.cpp's multilingual
`ggml-base.bin`. To build the daemon and CLI for Windows and Android from Linux, with whisper.cpp,
see [`tools/cross/README.md`](tools/cross/README.md).

For the web dashboard: `cd web && npm install && npm run dev`. For the Teams bot, see
[`teams-bot/README.md`](teams-bot/README.md).

## Running

```sh
export NOTE_TAKER_ENCRYPTION_PASSPHRASE='…'   # or rely on the platform keychain
target/debug/daemon                           # HTTP/WebSocket API on 127.0.0.1:8080
target/debug/cli meetings list
target/debug/cli record start --title "Weekly sync" --input meeting.wav
target/debug/cli record start --title "Standup"   # the microphone (--features microphone)
target/debug/cli log-level debug              # a running daemon applies it within 2 s
target/debug/cli devices list                 # devices this database syncs with
target/debug/cli devices revoke "My phone"    # a running daemon stops syncing with it within 1 s
```

The full CLI, HTTP, WebSocket, and C ABI contracts are in [API_SPECS.md](API_SPECS.md).

## Configuration

Everything is configured through environment variables. Secrets never live in files in the
repository.

| Variable | Default | Purpose |
| --- | --- | --- |
| `NOTE_TAKER_DATABASE_PATH` | `note_taker.sqlite3` | SQLite database. The control files `<db>.device-id`, `<db>.log-level`, and `<db>.recording-stop` sit beside it |
| `NOTE_TAKER_DEVICE_ID` | generated once, stored in `<db>.device-id` | Acquisition device ID stamped on notes and logs |
| `NOTE_TAKER_ENCRYPTION_PASSPHRASE` | unset | Derives the AES-256-GCM key with Argon2id. If unset, a random key is kept in the platform keychain or a key file |
| `NOTE_TAKER_KEYCHAIN_ENABLED` | `true` | `false`/`0`/`no`/`off` skips the keychain and uses the key file |
| `NOTE_TAKER_ENCRYPTION_KEY_DIR` | `~/.note_taker` | Directory of the fallback `encryption.key` |
| `NOTE_TAKER_LOG_DIR` | `logs` | Daily rolling JSON log files (`app_log.YYYY-MM-DD`) |
| `RUST_LOG` | `info` (daemon) | Initial daemon log filter. CLI: `--log-level` |
| `NOTE_TAKER_BIND_ADDRESS` | `127.0.0.1:8080` | Daemon HTTP/WebSocket address |
| `NOTE_TAKER_P2P_ENABLED` | `true` | Daemon pairing and sync with your other devices. `false` turns it off |
| `NOTE_TAKER_P2P_LISTEN` | `/ip4/0.0.0.0/tcp/0` | libp2p listen multiaddress (port 0 picks a free port). Only paired devices can connect, except during pairing |
| `NOTE_TAKER_DEVICE_NAME` | the host name | Name shown to your other devices during pairing |
| `NOTE_TAKER_P2P_PEERS` | unset | Comma-separated `…/p2p/<peer-id>` addresses of your devices that mDNS cannot find (another subnet, NAT, VPN). They appear as nearby for pairing. Paired devices, configured or found by mDNS, are redialed every 10 s while disconnected |
| `NOTE_TAKER_TRANSCRIPTION_PROVIDER` | `auto` | `auto`, `local` (whisper.cpp), or `openai` |
| `NOTE_TAKER_WHISPER_MODEL` | unset | Path to the local GGML model (`local-ai` builds) |
| `NOTE_TAKER_WHISPER_LANGUAGE` | `auto` | Language detection, or a code such as `it` or `en` |
| `OPENAI_API_KEY` | unset | Enables cloud transcription. Audio leaves the device only when this is set |
| `NOTE_TAKER_OPENAI_WHISPER_MODEL` | `whisper-1` | OpenAI transcription model |
| `NOTE_TAKER_OPENAI_LANGUAGE` | unset (auto) | Language hint for the OpenAI API |
| `NOTE_TAKER_VAD_ENABLED` | `true` | Voice-activity chunking. `false` cuts near every 15 s |
| `NOTE_TAKER_P2P_RELAY` | unset | `…/p2p/<peer id>` address of your relay, to reach devices across NATs. It must be paired with this device. In the apps, set it in Settings → Other networks instead; a saved value replaces this one |
| `NOTE_TAKER_P2P_RELAY_SERVER_ADDRESS` | unset | On your always-on daemon: its public address (`/ip4/203.0.113.5/tcp/4001`), which turns the relay on. Use a fixed port in `NOTE_TAKER_P2P_LISTEN` |
| `NOTE_TAKER_ENABLE_DIARIZATION` | `false` | Speaker diarization (`diarization` builds, with both models below). Otherwise segments are unattributed |
| `NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL` | unset | Path to `segmentation.onnx`, from [`tools/diarization`](tools/diarization/README.md) |
| `NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL` | unset | Path to `embedding.onnx`, from the same tool |
| `NOTE_TAKER_REVIEW_CLIP_DIR` | `voice_review_clips` | Temporary clips of unknown voices, kept until resolved |
| `NOTE_TAKER_EXPORT_DIR` | `~/Documents/NoteTaker/exports` | Export destination |
| `NOTE_TAKER_PDF_FONT_DIR` | unset | Extra fallback fonts for PDF export |
| `NOTE_TAKER_TEAMS_BRIDGE_ENABLED` | `false` | Starts the Teams bot bridge in the daemon |
| `NOTE_TAKER_TEAMS_BRIDGE_PORT` | `8765` | Bridge port (always bound to 127.0.0.1) |
| `NOTE_TAKER_TEAMS_BRIDGE_SECRET` | unset | Shared secret the Teams bot authenticates with |
| `NOTE_TAKER_TEAMS_BRIDGE_BUFFER_SECONDS` | `10` | Audio buffered before transcription (1–300) |
| `NOTE_TAKER_TEAMS_BRIDGE_SLIDE_DIR` | `slides` beside the database | Where Teams slide screenshots are stored |
| `NEXT_PUBLIC_API_BASE_URL` | `http://127.0.0.1:8080` | Web dashboard: daemon API |
| `NEXT_PUBLIC_WS_URL` | derived from the API URL | Web dashboard: live-transcript WebSocket |

## Project documents

- [Architecture](ARCHITECTURE.md)
- [Requirements](REQUIREMENTS.md)
- [API contract](API_SPECS.md)
- [v1.0 audit](AUDIT_V1.md)
- [Engineering context](CONTEXT.md)
- [Task board](TASK_BOARD.md)
- [Decision log](DECISION_LOG.md)
- [Contributing guidelines](CONTRIBUTING.md)
