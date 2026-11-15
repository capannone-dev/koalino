<!-- This directory contains infrastructure implementations of ports and operational cross-cutting concerns. -->

# Adapters layer

Adapters are the only layer allowed to integrate external systems. This layer provides observability,
local audio-capture, attachment metadata, storage, libp2p network adapters, and opt-in local AI
adapters. `ai/local_whisper_cpp.rs` is compiled only with `--features local-ai`; it implements
`TranscriptionEngine` without allowing whisper.cpp types into the application layer. The in-memory
voice-vector adapter is deliberately lightweight and test-oriented;
it performs cosine similarity locally and must be replaced by an encrypted-at-rest production
adapter without changing the application or domain boundaries.

`keychain/` implements the `KeychainStore` port with the `keyring` crate: the random
voice-embedding key is kept in the macOS Keychain, Windows Credential Manager, or Linux Secret
Service. See `keychain/README.md` for entry names and error classification.

`accessibility/` reads who call windows show as speaking (#113): over AT-SPI with zbus on
Linux and with UI Automation on Windows, only reading, within a node and time budget. See
`accessibility/README.md`.

`child_process.rs` handles waits for the core's child processes inside the desktop app. While the
Flutter engine has processes of its own running, it reaps children with `waitpid(-1)`, and it may
collect the status of a process the core started (`ECHILD`). `reaped_elsewhere` recognises that
case. The microphone-user probe (#111) and the Claude Code / Codex runner (#105) then keep the
output they read and treat the status as unknown, instead of reporting a failure.

`documents/` reads the text of attached files (PDF, Word, PowerPoint, Excel, LibreOffice, e-mails,
text) for summaries, chat, hand-offs and search (#128). See `documents/README.md`.

`email/` implements the `EmailReader` port with the pure-Rust `mail-parser` crate, for e-mails kept in projects
(#85). See `email/README.md`.

`export/` implements the `MeetingExporter` port for Markdown, PDF (`printpdf` with its bundled
`lopdf`), and versioned JSON. PDFs embed the bundled DejaVu Sans fonts, plus CJK and emoji fallback
fonts from `NOTE_TAKER_PDF_FONT_DIR`, `~/.note_taker/fonts/`, or the system.
`export::default_exporters()` supplies all three exporters to composition roots, and
`export::default_export_directory()` resolves `NOTE_TAKER_EXPORT_DIR` or
`~/Documents/NoteTaker/exports`. See `export/README.md` for formats, PDF fonts, and the JSON schema.
