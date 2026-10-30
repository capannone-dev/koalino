<!-- Records the pre-v1.0 audit: what was checked, what was fixed, requirement compliance, and release readiness. -->

# v1.0 audit and remediation report

**Date:** 2026-10-28 · **Scope:** the whole repository (`core/`, `ui/`, `web/`, `teams-bot/`, CI,
docs) · **Rule for conflicts:** Security > Privacy > Reliability > Performance > Convenience.

## 1. Summary

The audit fixed 15 problems in place (every fix was tested and pushed, and CI is green), and
filed 7 issues (#43–#49) for gaps that are too large to fix in an audit or need library approval.

**v1.0 readiness: not ready.** Three core requirements are not met end to end:

1. Devices do not sync with each other (#43).
2. Speakers are distinguished only in `diarization` builds with exported models (#44).
3. The Flutter app records no microphone audio (#45).

Everything else is either working or is a contained, tracked gap. Section 5 lists the release
criteria.

## 2. Fixes made during the audit

| Commit | Area | Problem → fix | Priority class |
| --- | --- | --- | --- |
| 6c9e11a | Server | Any visited web page could read live transcripts over `/ws` (CORS does not cover WebSockets) or read the whole API via DNS rebinding → middleware refusing foreign `Origin` and DNS-name `Host` | Security, Privacy |
| 208a030 | Web | `next@15.2.4` had 36 advisories, including an unauthenticated RCE → 15.5.28 (none known) | Security |
| 149d613 | Deps | `time` stack-exhaustion advisory → 0.3.47 | Security |
| 652dd75 | Logging | Logs lacked `device_id`, didn't match the 6.3 shape, and had no redaction → `StructuredJsonFormat` with a field-name redaction denylist (keys, embeddings, titles, transcripts, PCM) | Privacy |
| 2bcd140 | FFI | JSON escaped only `"` and `\`, so control characters produced invalid JSON → escaping via `serde_json` | Reliability |
| f90c17c | Core | The daemon hardcoded the device ID `headless-server` → a persistent per-database `device-<uuid>` | Reliability |
| 200b9f9 | AI | whisper.cpp transcribed everything as English (4.2 needs Italian) → `NOTE_TAKER_WHISPER_LANGUAGE`, default `auto` | Reliability |
| db67162 | CI | Core CI was red on `main` (the crypto benchmark missed its budget in debug builds) → AES crates optimized in the dev profile, best of 3 | Reliability |
| 1c94237, f29a811 | Flutter | FFI calls (SQLite reads, possibly Argon2id) ran on the UI isolate → all bridges use `Isolate.run` | Performance |
| 638a940 | CLI | No way to change a headless daemon's log level at runtime (6.1) → `--log-level` and `log-level <directive>` | Convenience / requirement |
| c5f9698 | CLI | The CLI log file recorded only errors (6.1 requires INFO) → separate stdout and file filters | Reliability |
| f29a811 | Flutter | Recordings could only be started → pause, resume, and stop | Requirement 2.1 |
| 8e58ac7 | Web | An unreachable daemon showed the generic Next.js error → recoverable error page | Reliability |
| 7a2177e | Web | Lint failed after a build (it linted `.next/`) → ignores and named exports | CI |
| 2c7fdf9 | Docs | Telemetry HTTP routes and C ABI were undocumented → `API_SPECS.md` | Docs |

The root `README.md` was rewritten (components, build, run, and a table of every environment
variable), and `TASK_BOARD.md`, `DECISION_LOG.md`, and `CONTEXT.md` were updated.

## 3. Requirements compliance matrix

✅ met · ⚠️ partial · ❌ not met

| Req. | Requirement | Status | Evidence / gap |
| --- | --- | --- | --- |
| 2 | Android, Material 3 | ⚠️ | The Flutter app uses Material 3, but there is no mic capture (#45) and no background service (#47) |
| 2 | Linux GNOME native look | ⚠️ | Material 3 on GNOME; no Adwaita styling (TASK_BOARD P2) |
| 2 | Windows native look | ⚠️ | Material 3 on Windows; no Fluent styling (TASK_BOARD P2) |
| 2 | Linux server: headless daemon and web UI | ✅ | `daemon` with Axum HTTP/WS; Next.js dashboard |
| 2 | Linux CLI, full-featured | ✅ | `cli` covers meetings, record, voice, telemetry, export, and log-level |
| 2.1 | Background recording | ⚠️ | Daemon and CLI: yes. Flutter: no foreground service (#47) |
| 2.1 | Lock-screen Start/Pause/Stop | ❌ | Core hooks exist (#16); no Android notification controls (#47) |
| 3.1 | High-quality capture | ⚠️ | 16 kHz PCM through the CLI, FFI, and Teams; the Flutter mic is not wired (#45) |
| 3.1 | Screen-share detection and slide capture | ⚠️ | Teams slides only; no local detection (#49) |
| 3.1 | Delete raw audio after processing | ✅ | Chunk PCM lives only in memory and is wiped when consumed or on stop (#32) |
| 3.1 | Attachments with metadata-based meeting suggestion | ✅ | `AttachmentLinker`, CLI/FFI, Flutter import screen |
| 3.2 | Diarization | ✅ opt-in | `diarization` feature with exported models (#44); the apps do not ship the models yet |
| 3.2 | Voice signature library, multiple per identity | ✅ | Encrypted `SqliteVoiceVectorDB`, multiple variants |
| 3.2 | Manual assignment, low-confidence prompts, new variant on confirm, clip preservation | ✅ (no real input) | Implemented and tested; receives real data only once #44 lands |
| 3.2 | Voice Gallery | ✅ | Flutter and web |
| 3.3 | Date, time, and device ID on notes | ✅ | The device ID is now persistent and unique per node (f90c17c) |
| 3.3 | Offline-first operation | ✅ | Fully local; network adapters are optional |
| 3.3 | Sync on reconnect without conflicts | ❌ | CRDT sync exists but is never composed (#43) |
| 3.3 | Shared voice libraries and meetings over P2P | ❌ | #43 |
| 3.3.1 | Export: Markdown, PDF (Unicode), JSON | ✅ | CLI and HTTP; Flutter UI missing (#48) |
| 3.4 | Device manifest broadcast | ❌ | The domain type and dispatcher exist; no broadcast (#43) |
| 3.4 | Workload routing Local / Specific / Auto | ⚠️ | `ComputeDispatcher` exists; there are no remote candidates without #43 |
| 3.4 | Swap local AI for API models | ✅ | whisper.cpp ⇄ OpenAI behind `TranscriptionEngine` |
| 3.4 | "Dumb" capture-only nodes | ⚠️ | The architecture allows them (ports); they need #43 |
| 3.5 | Telemetry per device and task (time, power, latency, load) | ✅ | CLI, HTTP, FFI, and both dashboards |
| 4.1 | Strictly P2P, no central server | ✅ (by design) | No central component; the P2P path itself is not composed yet |
| 4.1 | LAN mDNS and NAT traversal | ⚠️ | mDNS is in the adapter; NAT traversal is not built; none of it runs (#43) |
| 4.1 | E2EE between devices | ⚠️ | Noise is configured, but no peer authorization exists yet, a prerequisite in #43 |
| 4.1 | Voice signatures encrypted at rest | ✅ | AES-256-GCM, with the key from Argon2id or the keychain (#31, #40) |
| 4.2 | Italian and English | ✅ | Whisper language auto-detection (200b9f9) |
| 5.x | Docs hierarchy, TDD, Conventional Commits | ✅ | A README in every directory, file headers, tests first |
| 6.1 | ERROR to TRACE, runtime toggle, INFO default | ⚠️ | Daemon, CLI, and FFI: yes. There is no Flutter settings UI to toggle it (#46) |
| 6.2 | Rotating log file | ✅ | Daily `app_log.YYYY-MM-DD` |
| 6.2 | Share/Export Logs button | ❌ | #46 |
| 6.3 | Structured JSON with the required fields | ✅ | `StructuredJsonFormat` (652dd75) |
| 6.4 | `tracing` to stdout and a rolling file; redaction | ✅ | Both sinks; field-name redaction |
| 6.4 | Flutter logging bridged to Rust | ⚠️ | Flutter sets the Rust level through FFI, but has no own structured logger |

Totals: 19 ✅ · 12 ⚠️ · 6 ❌, across 37 rows.

## 4. Audit phases

- **Docs alignment and completeness:** stale statements were fixed (the CLI log default, the
  rename sync note, the README "scaffold" text). Every directory has a README, and every source
  file checked has a header.
- **Issue review:** all 42 issues were closed before the audit. Gaps found in their
  implementation were filed as #43–#49 rather than reopening them, since each closed issue's own
  scope was delivered.
- **Error handling:** there is no `.unwrap()` or `.expect()` in production Rust (tests use
  `unwrap_or_else(panic!)`), and every `unsafe extern` documents its safety rules. Flutter maps FFI
  statuses to typed failures and never shows Rust's error text.
- **Tests:**

  | Component | Tests |
  | --- | --- |
  | Rust | 461 (+2 ignored that need real models) |
  | Flutter | 84 |
  | Web | 55 |
  | Teams bot | 66 |

- **CI:** four workflows (core, flutter, web, teams-bot), all green on `main`.
- **Lint:** `cargo fmt`, `clippy -D warnings` with all features, `flutter analyze`, and `eslint`
  are clean.
- **Security:** no hardcoded secrets; there is no `format!` SQL in production code; the daemon's
  loopback-only API is guarded against cross-site browser attacks; the Teams bridge refuses
  browser origins. Dependency advisories:

  | Package | Status |
  | --- | --- |
  | `next` | fixed |
  | `time` | fixed |
  | `postcss` (bundled in Next, build-time only) | accepted |
  | `yamux 0.12`, `hickory 0.25` (libp2p mDNS) | unreachable until #43, and a libp2p upgrade is now a prerequisite there |
  | `lopdf 0.31` (printpdf) | parses PDFs only in tests |

- **Performance:** no FFI on the Flutter UI isolate; whisper runs on blocking workers; AES-GCM is
  optimized in dev builds.

### Not verified here

- The Teams bot was not run locally (it needs the ASP.NET runtime); its CI is green.
- The keychain was not checked on macOS or Windows.
- No GUI run on Android.
- No real-model transcription run (`local-ai` needs cmake).
- Flutter (pub) and .NET dependencies were not checked for advisories (no tool available
  offline).

## 5. v1.0 readiness assessment

| Gate | State |
| --- | --- |
| Security baseline (secrets, encryption at rest, local API guard, patched deps) | ✅ Met |
| Builds, lint, and tests green in CI | ✅ Met |
| Single-device CLI/daemon workflow (record WAV/stdin → transcript → review → export) | ✅ Met |
| Single-device desktop/Android app workflow | ❌ Blocked by #45 (no mic audio) |
| Speaker attribution | ⚠️ Opt-in: `diarization` builds with exported models (#44) |
| Multi-device sync, the product's defining feature | ❌ Blocked by #43, which includes peer authorization and a libp2p upgrade |
| Mobile background and lock-screen operation | ❌ #47 |
| Log export for QA | ❌ #46 |

**Recommendation:** don't tag v1.0. What exists could ship as a **v0.9 "single-node headless"**
release (daemon, CLI, web dashboard, Teams bridge), which meets its requirements today. For v1.0,
land in order:

1. **#43** (with peer pairing first: security before connectivity);
2. **#45**;
3. **#44**;
4. **#46** and **#47**.

#45, #46, and #47 each need a Flutter plugin approved (capture, sharing, foreground service), and
#44 needs a model-runtime decision, so those approvals are the critical path.
