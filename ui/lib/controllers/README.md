<!-- This directory contains the Dart-package recording state controller. -->

# Controllers

`RecordingController` maps bridge success or failure into displayable recording state. It contains
no `dart:ffi` code and can be exercised through an in-memory bridge fake.

`AttachmentController` owns selected-file workflow state and decodes the Rust-owned JSON array of
meeting candidates. It receives both a picker and `AttachmentBridge` interface, preserving
widget-test injection and ensuring no complex Rust struct crosses FFI.

`RecordingController.startRecording(title:)` normalizes a blank title to `null` and exposes the
active title. `MeetingListController` owns the meeting list, the selected meeting's detail, and
their loading and error states. One controller serves both the desktop list-detail layout and
the phone detail route, and it drops a detail response that arrives after a newer selection
(Issue #36).

`meeting_title_validation.dart` mirrors Rust's `MeetingTitle::parse`: it trims, then rejects
empty titles, titles over 200 Unicode characters, and control characters, with a message for each.
`MeetingListController.renameMeeting` sends the trimmed title and, on success, replaces the
summary in the list and the selected detail without reloading. It rethrows failures and reloads
the list when the meeting no longer exists (Issue #41).

`DevicesController` holds the P2P status and runs the pairing commands: make discoverable, pair,
confirm codes, cancel, accept, reject, and remove. It refreshes after each success and maps Rust
error codes to plain messages, keeping the last good status. It polls every 2 s while the Devices
destination is shown. `discoverable` is true for the five-minute pairing window, measured with
`package:clock`, so tests can fake time without leaving a timer pending (Issue #43).

`CoreStartupController` runs `startCore` and maps Rust's status to `ready`, `keychainUnavailable`
(status 6), or `failed`. A throwing start, such as a missing library, also counts as `failed`.
`start` can be called again to retry.

`AppLockController` (Android) decides when the app lock covers the app: at start-up and after
the app was out of sight for `lockAfterBackground` (30 s), while the setting is on and the device
has a screen lock. A failure to read the setting keeps the app locked. The prompt is shown once
per lock automatically, and again from Unlock. The app being hidden by the prompt's own PIN
screen does not lock it again. Turning the setting on or off needs a successful prompt first.
Only the UI is locked: the Rust core, recording, and sync keep running.
