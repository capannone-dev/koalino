<!-- This directory contains the Flutter client and its adapter-only boundary to the Rust core. -->

# Flutter client

The Flutter client renders application state and delegates native work to `infrastructure/`.
Presentation widgets never import Rust-generated or `dart:ffi` bindings. Controllers depend on
small Dart bridge interfaces, allowing widget tests to use deterministic fakes.

## Navigation

`HomeShell` hosts the Record, Meetings, Devices, and Settings destinations; Telemetry is a page
of Settings (Issue #53). Windows narrower than 840 px
(phones) use a Material 3 `NavigationBar`, and wider ones (desktops, tablets in landscape) use a
`NavigationRail`. Only the selected destination is built. Meetings and the Telemetry page each own
their controller and dispose it when left, so the telemetry timer stops and the meeting list
reloads on every visit. The app-lifetime `RecordingController` keeps an active recording across
destinations. This replaces the earlier drawer, whose push used a context above the navigator.

## Recording controls

`RecordingScreen`'s status panel is itself the **Start recording** button before a recording (tap
or Enter; there is no separate start button), then it offers **Pause** and **Stop** while recording, and
**Resume** and **Stop** while paused. Resume calls `note_taker_start_recording`, which continues
the paused meeting (a titled start would create a new one). Stop shows "Finishing
transcription…" until Rust has transcribed and persisted every chunk, then "Recording saved" and
clears the title. A failed pause, resume, or stop keeps the current state and shows an error.

`RustRecordingBridge` runs start, pause, and stop on `Isolate.run`, as the Rust contract requires:
the first start may load a model and stop waits for transcription, so calling them on the UI
isolate froze the app. Each isolate looks the symbol up itself, because a `DynamicLibrary`
cannot cross isolates.

Microphone capture is not wired yet: nothing feeds `note_taker_push_audio_pcm`, so a recording
started from the app receives no audio (see TASK_BOARD.md).

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

## Renaming meetings (Issue #41)

- **Where:** in the meeting list, long-press a row (touch), right-click it (mouse), or press
  Shift+F10 or the context-menu key on a focused row, then choose **Rename**. Screen readers get
  the same **Rename** custom action. In the meeting detail, the pencil next to the title opens
  the same dialog.
- **Dialog:** `RenameMeetingDialog` is a Material 3 `AlertDialog` with the current title
  pre-filled and selected. Enter saves and Escape or **Cancel** closes. The field is 400 px wide,
  narrower on small phones. The counter counts Unicode characters, as Rust does.
- **Validation:** `validateMeetingTitle` applies Rust's `MeetingTitle::parse` rules before any
  FFI call: trim, then reject empty titles ("Enter a title."), titles over 200 Unicode
  characters (an emoji counts once), and control characters (U+0000–U+001F, U+007F–U+009F).
  Errors appear after the first Save and then update as the person types. Saving an unchanged
  title just closes the dialog.
- **Bridge:** `RustMeetingBridge.renameMeeting` calls `note_taker_rename_meeting` in
  `Isolate.run` and maps non-zero statuses to `MeetingRenameException`: 1 `invalidTitle`, 2
  `storageFailure`, 3 `runtimeUnavailable`, 5 `meetingNotFound`, any other `unknown`. The
  dialog stays open and shows a plain message for each.
- **State:** `MeetingListController.renameMeeting` updates the list row and the open detail in
  place, without reloading, so the desktop two-pane layout updates both. A "Meeting renamed"
  snack bar confirms. If the meeting was deleted meanwhile, the list reloads.

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
- `note_taker_get_voice_library` tags each voice with `voice_group_id`. Rust decides which voices
  sound like one person (#82); Flutter only collects them into one card. The card names its ticked
  voices with `note_taker_assign_voice_group`, and a voice can be unticked when it is someone else.

Both calls read and write SQLite synchronously, so the bridge runs each one in `Isolate.run` from
a top-level function that opens the core library inside the isolate. `decodeUnknownClusters` and
`decodeClusterAssignment` turn the JSON, including Rust's `{"ok":false,...}` errors, into Dart
values or a `StateError`. A blank name is rejected in the controller without calling Rust.

The clip-based weak-match and new-identity UI was replaced by this cluster view (Issue #29). The
older clip FFI symbols remain exported, but the Dart bridge no longer binds them.

## Telemetry dashboard

Settings → Telemetry opens a responsive Material 3 telemetry dashboard for mobile and desktop. The
pipeline records one row per transcribed chunk (Issue #53). The dashboard polls
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
