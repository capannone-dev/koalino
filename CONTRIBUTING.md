<!-- The project's contribution rules: stack, coding and documentation standards, security and privacy rules, commits, and how issues are closed. Other documents cite its sections by number (for example §7). -->

# Contributing to Koalino

Thank you for helping. These are the rules the code base follows. Other documents cite them by
section number (for example "CONTRIBUTING.md §7"), so the numbering is kept stable.

## 1. About this document

It applies to every change: code, documentation, tests and packaging. When a change needs an
exception (for example keeping raw audio, §7), record the decision and the reason in
[`DECISION_LOG.md`](DECISION_LOG.md).

## 2. Project context

Koalino is a decentralized, offline-first meeting note-taker. Devices pair over a P2P network
(libp2p), sync notes through CRDTs, route AI work to the device best able to run it, and keep voice
signatures (biometric data) secure. See [`ARCHITECTURE.md`](ARCHITECTURE.md).

## 3. Tech stack and constraints

- **Core logic:** Rust.
- **Desktop and mobile UI:** Flutter (Dart).
- **Linux server and web UI:** Rust back end plus a React/Next.js front end.
- **Networking:** libp2p. No custom socket implementations.
- **Sync and storage:** CRDTs (Automerge) plus SQLite.
- **AI models:** Whisper (transcription), Pyannote-style diarization, speaker embeddings.
- New libraries must be compatible with GPL-3.0-or-later and listed in
  [`THIRD_PARTY_NOTICES.md`](THIRD_PARTY_NOTICES.md).

## 4. Workflow

1. **Understand:** read the relevant `README.md`, `ARCHITECTURE.md`, `DECISION_LOG.md` and the code.
2. **Plan:** open or pick a GitHub issue and describe the approach there.
3. **Test first:** write the failing test before the implementation.
4. **Implement:** make the test pass.
5. **Document:** add the documentation required by §6.
6. **Verify:** all tests pass and the linters report no errors (§10).

## 5. Coding standards

### Rust (core)

- `anyhow` for application errors, `thiserror` for library errors.
- **No `.unwrap()` or `.expect()` in production code.** Use `?`, `match` or `if let`.
- Strict typing: prefer `&str` or specific enums over `String` where they fit.
- `cargo fmt`, and `cargo clippy` with zero warnings.

### Flutter (UI)

- Strict null safety, Material 3.
- Keep widgets simple; push logic to the Rust core through FFI or the local API.

## 6. Documentation requirements

Code without documentation is incomplete.

- **Folders:** every directory has a `README.md` explaining its purpose, contents and place in the
  architecture.
- **Files:** every file starts with a header comment describing its purpose.
- **Functions and methods:** doc comments state what it does, its inputs, its outputs (including
  errors) and its side effects (for example "writes to SQLite", "broadcasts over P2P").
- **Names:** descriptive; add an inline comment where a name is not self-explanatory.

## 7. Security and privacy

- **Encryption in transit:** all P2P traffic uses libp2p's encrypted transports. Never plaintext
  over a network.
- **Data at rest:** voice signatures (biometrics) and sensitive notes are encrypted at rest with
  AES-256-GCM.
- **Secrets:** never hardcode API keys, passwords or cryptographic seeds. Use environment variables
  or the system keychain.
- **PII:** raw audio is deleted after processing unless explicitly kept. Voice embeddings are PII
  and are handled securely.

## 8. Commits

- Small, atomic, incremental commits.
- [Conventional Commits](https://www.conventionalcommits.org/) messages, for example
  `feat(p2p): add mDNS discovery` or `fix(ui): resolve state leak in note view`.
- The body explains what changed and why.
- All tests pass before a commit.

## 9. What not to do

- No centralized server for P2P routing: the network stays decentralized.
- Do not block the Flutter UI thread or the Rust async runtime with heavy AI work: run it on
  background threads or tasks.
- Do not store raw audio for voice signatures: only the embeddings (vectors).
- Do not assume the network is available: the app works fully offline and syncs later.

## 10. Verification commands

- **Format and lint (Rust):** `make rust-lint` (`cargo fmt --check` and `cargo clippy`).
- **Format and analyze (Flutter):** `make flutter-lint`.
- **Unit tests (Rust):** `make rust-test` (`cargo test --lib`, with mocked AI models).
- **UI tests (Flutter):** `make flutter-test`.
- **Desktop build:** `make build-desktop`. **Server build:** `make build-server`.

Run the relevant lint and test commands before opening a pull request.

## 11. Project records

- [`DECISION_LOG.md`](DECISION_LOG.md): architectural decisions and their reasons. Read it before
  changing a component, and add an entry for every significant decision.
- [`TASK_BOARD.md`](TASK_BOARD.md): the state of the planned work.

## 12. Linking issues

A commit that completes an issue includes `Closes #<number>` in its message, for example
`feat(cli): implement voice management commands (Closes #18)`. A commit that only partly addresses
an issue references it without the keyword (`Refs #<number>`).

## 13. Closing an issue

When an issue is done:

1. The commit message includes `Closes #<number>`.
2. A comment on the issue summarizes the implementation: what was added, how it was tested, and
   the commit hash and subject.
