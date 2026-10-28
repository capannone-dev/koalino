<!-- This directory owns Dart's concrete boundary adapter to the Rust C ABI. -->

# FFI infrastructure

The recording bridge converts Dart calls to primitive C ABI functions and status codes. Rust
domain structs are never represented in Dart or presented to widgets.

`attachment_bridge.dart` wraps the standard `file_picker` plugin behind `AttachmentFilePicker` and
maps its selected local path to the narrow Rust C ABI. Suggestions return as caller-freed JSON;
the bridge deliberately leaves JSON parsing to `AttachmentController`.
Both attachment calls run on a background isolate, because scoring reads every meeting from
SQLite.

`telemetry_bridge.dart` defines the telemetry DTOs and `RustTelemetryBridge`, which calls
`note_taker_get_devices`, `note_taker_get_telemetry_summary`, and
`note_taker_get_device_telemetry` on a background isolate and frees each response there.

Rule for every bridge: no native call runs on the UI isolate. Each worker isolate opens the
library and looks up its symbols itself, because `DynamicLibrary` handles and native pointers
can't cross isolates; only Dart strings and ints come back.

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

`app_configuration.dart` runs first: `configureCore` passes the application-support directory
(`<support>/note_taker`), the device name (the host name; "Android device" on Android), and
`models/ggml-base.bin` when present to `note_taker_configure` on a background isolate. Android
sets `keychain_enabled: false`, so the key file stays in the app-private `keys/` directory.
`appConfigurationJson` builds the JSON and is unit-tested.

`p2p_bridge.dart` defines `P2pStatus` and its rows, `P2pCommandResult`, and `P2pException` (with
Rust's `error_code`), the pure decoders `decodeP2pStatus` and `decodeP2pCommandResult`, and
`p2pCommandJson`. `RustP2pBridge` calls `note_taker_p2p_status` and `note_taker_p2p_command` on
background isolates; a command blocks its isolate until the P2P task answers (Issue #43).
