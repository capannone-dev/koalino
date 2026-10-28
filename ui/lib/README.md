<!-- This directory is the Dart package entry point for the Flutter client. -->

# Flutter package sources

The package exposes the application entry point and separates presentation, controller, and FFI
infrastructure code. Dart package imports resolve only from this directory.

Meeting attachments (Issue #54) span all three layers:
- `infrastructure/`:
  - `meeting_attachment_bridge.dart` holds the multi-file picker and `note_taker_attach_files`;
  - `photo_camera.dart` wraps the `camera` plugin.
- `controllers/`: `MeetingAttachmentsController` and `PhotoCaptureController`.
- `presentation/`:
  - `photo_capture_screen.dart`, the multi-shot camera;
  - `meeting_timeline.dart`, which places attachments among the transcript's segments.

The camera is offered on Android, iOS and Windows. Linux has no `camera` implementation, so it
attaches files only.

Meeting rename (Issue #41) spans all three layers: `controllers/meeting_title_validation.dart`
mirrors Rust's title rules, `MeetingListController.renameMeeting` updates state in place,
`presentation/rename_meeting_dialog.dart` is the shared dialog, and `RustMeetingBridge` calls
`note_taker_rename_meeting`.
