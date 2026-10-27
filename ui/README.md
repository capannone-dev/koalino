<!-- This directory contains the Flutter client and its adapter-only boundary to the Rust core. -->

# Flutter client

The Flutter client renders application state and delegates native work to `infrastructure/`.
Presentation widgets never import Rust-generated or `dart:ffi` bindings. Controllers depend on
small Dart bridge interfaces, allowing widget tests to use deterministic fakes.

## Navigation

`HomeShell` hosts the Record, Meetings, and Telemetry destinations. Windows narrower than 840 px
(phones) use a Material 3 `NavigationBar`, and wider ones (desktops, tablets in landscape) use a
`NavigationRail`. Only the selected destination is built. Meetings and Telemetry each own their
controller and dispose it when left, so the telemetry timer stops and the meeting list reloads
on every visit. The app-lifetime `RecordingController` keeps an active recording across
destinations. This replaces the earlier drawer, whose push used a context above the navigator.

## Meeting titles (Issue #36)

- **Recording:** `RecordingScreen` has an optional **Meeting title** field ("Enter meeting
  title (optional)", at most 200 characters; Enter starts the recording).
  - `RecordingController.startRecording(title:)` trims the title and sends a blank one as
    `null`.
  - `RustRecordingBridge.startRecordingWithTitle` passes it to
    `note_taker_start_recording_with_title` with a null meeting ID. Rust rejects an empty
    string, so blank titles never become `""`.
  - The field is locked while recording, and the status reads `Recording "<title>"`.
  - The content is capped at 480 px wide and scrolls above the on-screen keyboard.
- **Meeting list:** `MeetingListScreen` lists meetings newest first.
  - Each row shows the title (bold `titleMedium`) above `YYYY-MM-DD HH:MM UTC · M:SS`.
  - Untitled meetings show Rust's fallback, `Meeting YYYY-MM-DD HH:MM UTC`. The decoder
    rebuilds the same fallback if a response ever lacks a title.
  - Phones push `MeetingDetailScreen`. From 840 px the list and `MeetingDetailView` sit side by
    side, and the selected row is highlighted.
  - Includes pull-to-refresh, a refresh action, and empty and error (with **Retry**) states.
- **Meeting detail:** shows, in order:
  - the title as a bold `headlineMedium` semantic header;
  - the date and duration;
  - the selectable meeting ID;
  - the speaker-attributed segments (`speaker · start`), the flat transcript, or "No
    transcript is available."

  The app bar's paperclip opens `AttachmentImportScreen`.
- **Bridge:** `RustMeetingBridge` calls `note_taker_get_meetings` and `note_taker_get_meeting`
  in `Isolate.run`, because they read SQLite synchronously. The pure functions `decodeMeetings`
  and `decodeMeetingDetail` parse the JSON (`title`, `created_at`, `duration_ms`, `segments`)
  and turn Rust's `{"ok":false,...}` errors into a `StateError`. Controllers show generic
  messages, because Rust errors can contain storage paths.

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

The Telemetry destination opens a responsive Material 3 telemetry dashboard for mobile and desktop. It polls
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
