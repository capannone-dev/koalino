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
