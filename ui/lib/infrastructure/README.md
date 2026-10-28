<!-- This directory owns Dart's concrete boundary adapter to the Rust C ABI. -->

# FFI infrastructure

The recording bridge converts Dart calls to primitive C ABI functions and status codes. Rust
domain structs are never represented in Dart or presented to widgets.

`attachment_bridge.dart` wraps the standard `file_picker` plugin behind `AttachmentFilePicker` and
maps its selected local path to the narrow Rust C ABI. Suggestions return as caller-freed JSON;
the bridge deliberately leaves JSON parsing to `AttachmentController`.

`RustRecordingBridge` binds start (also resume), pause, and stop, and runs each on a background
isolate. `startRecordingWithTitle` calls `note_taker_start_recording_with_title`,
sending a null pointer for the meeting ID and for a blank title. `meeting_bridge.dart` defines the
`MeetingSummary`, `MeetingDetail`, and `MeetingTranscriptSegment` models, and the pure JSON
decoders with their fallback-title and formatting helpers. It also defines `RustMeetingBridge`,
which calls `note_taker_get_meetings` and `note_taker_get_meeting` on a background isolate and
frees each response with `note_taker_free_string` (Issue #36).

`RustMeetingBridge.renameMeeting` calls `note_taker_rename_meeting` on a background isolate, frees
both native strings, and maps non-zero statuses to `MeetingRenameException` through
`meetingRenameExceptionFor` (1 invalid title, 2 storage, 3 runtime, 5 not found, other unknown)
(Issue #41).
