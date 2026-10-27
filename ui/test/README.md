<!-- This directory holds widget tests that define Flutter behavior before presentation implementation. -->

# Widget tests

Tests inject a `RecordingBridge` fake into the controller, proving that widgets make state changes
without loading a native Rust library or accessing Rust implementation types.

`attachment_import_screen_test.dart` injects fake picker and FFI boundaries. It specifies that the
paperclip action opens the picker, JSON candidates render in rank order with confidence scores,
and Select forwards the exact opaque attachment and meeting IDs to Rust.

Meeting-title tests (Issue #36):

- `recording_screen_test.dart` covers the title field, its hint, sending a trimmed title or
  `null`, keyboard submit, locking while recording, and phone/desktop sizing.
- `meeting_bridge_test.dart` covers JSON decoding (titles, fallbacks, durations, segments, and
  error objects) and the formatting helpers.
- `meeting_list_screen_test.dart` checks custom and fallback titles, the title placed above and
  larger than its metadata, phone navigation, the desktop two-pane layout, and the empty and
  error states.
- `meeting_detail_screen_test.dart` checks the bold headline header (also as a semantic
  header), the transcript, the fallback title, load errors, and the paperclip action.
- `home_shell_test.dart` checks the adaptive navigation.

`fake_meeting_bridge.dart` holds the shared fake and fixtures.
