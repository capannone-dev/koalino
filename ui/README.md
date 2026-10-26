<!-- This directory contains the Flutter client and its adapter-only boundary to the Rust core. -->

# Flutter client

The Flutter client renders application state and delegates native work to `infrastructure/`.
Presentation widgets never import Rust-generated or `dart:ffi` bindings. Controllers depend on
small Dart bridge interfaces, allowing widget tests to use deterministic fakes.

## Voice Gallery

`VoiceGalleryScreen` renders `VoiceGalleryController` state for a meeting's unresolved diarization
clips. The controller depends on `VoiceGalleryBridge`; `RustVoiceGalleryBridge` is the only Dart
layer that calls the Rust C ABI. `note_taker_get_unassigned_clips` returns UTF-8 JSON metadata:
clip UUID, time range, optional weak-match candidate, and an absolute `local_file_path`. It never
returns audio bytes or biometric embeddings. The concrete bridge passes that native path to
`audioplayers`, so platform audio code reads the local temporary review file directly.

Weak-match confirmation and new-identity actions send only clip and identity/name strings back to
Rust. Rust retrieves retained derived embeddings from local storage and delegates variant creation
to `VoiceIdentityManager`; Flutter never receives or constructs biometric data.

## Telemetry dashboard

The drawer opens a responsive Material 3 telemetry dashboard for mobile and desktop. It polls
caller-freed Rust JSON every ten seconds and uses `fl_chart` for privacy-safe duration, latency,
task-mix, and power views; no audio, transcript, biometric, or credential content crosses FFI.

## Attachment import

`AttachmentImportScreen` provides the Meeting Detail paperclip action. Its controller obtains a
PDF or image path from the injected `AttachmentFilePicker`, asks `AttachmentBridge` for an owned
JSON array, and decodes that JSON in Flutter into display-safe ranked candidates. Widgets never
load Rust types or parse native pointers. `RustAttachmentBridge` frees every returned string after
copying it and sends only the opaque attachment and meeting IDs after the user presses **Select**.

The Rust boundary delegates ranking to `AttachmentManager`; its filesystem metadata adapter reads
only a local modification timestamp and its SQLite adapter persists metadata only after explicit
user approval. Attachment file contents never cross FFI or enter SQLite through this workflow.
