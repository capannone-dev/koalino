<!-- This directory contains the Flutter client and its adapter-only boundary to the Rust core. -->

# Flutter client

The Flutter client renders application state and delegates native work to `infrastructure/`.
Presentation widgets never import Rust-generated or `dart:ffi` bindings. Controllers depend on
small Dart bridge interfaces, allowing widget tests to use deterministic fakes.

## Voice Gallery

`VoiceGalleryScreen` renders `VoiceGalleryController` state for a meeting's unresolved
unknown-speaker clusters. Each card shows the cluster's total speech time and chunk count, a
**Play** button when Rust supplied a sample clip, a name field, and an **Assign** button. The
controller depends on `VoiceGalleryBridge`. `RustVoiceGalleryBridge` is the only Dart layer that
calls the Rust C ABI.

- `note_taker_get_unknown_clusters` returns a UTF-8 JSON array of cluster summaries with an
  optional absolute `review_clip_path`. The bridge passes that path to `audioplayers`, so platform
  audio code reads the local review file directly.
- `note_taker_assign_cluster` receives only the cluster ID and the typed name. Rust resolves the
  name, enrolls the cluster's embeddings, relinks transcript segments, and deletes the review
  audio. Flutter never receives or constructs biometric data.

Both calls read and write SQLite synchronously, so the bridge runs each one in `Isolate.run` from
a top-level function that opens the core library inside the isolate. `decodeUnknownClusters` and
`decodeClusterAssignment` turn the JSON, including Rust's `{"ok":false,...}` errors, into Dart
values or a `StateError`. A blank name is rejected in the controller without calling Rust.

The clip-based weak-match and new-identity UI was replaced by this cluster view (Issue #29). The
older clip FFI symbols remain exported, but the Dart bridge no longer binds them.

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
