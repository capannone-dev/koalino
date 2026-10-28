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

Meeting rename tests (Issue #41):

- `meeting_title_validation_test.dart` covers trimming, empty input, the 200-character limit
  counted in Unicode characters, control characters, and CJK/emoji titles.
- `meeting_list_controller_test.dart` covers in-place list and detail updates, rethrown
  failures, and the reload after "meeting not found".
- `meeting_bridge_test.dart` maps each rename status code to its failure.
- `meeting_list_screen_test.dart` opens the menu by long-press, right-click, Shift+F10, and the
  semantics action. It also covers validation errors, saving (button and Enter), unchanged
  titles, Cancel and Escape, every Rust status message, the desktop detail pane, and dialog sizing.
- `meeting_detail_screen_test.dart` covers the pencil's placement, the pre-filled dialog, and
  success and failure.

`fake_meeting_bridge.dart` holds the shared fake and fixtures; it records rename requests and
list loads, and can be told to fail a rename.

Devices (Issue #43):

- `p2p_bridge_test.dart` decodes the status, command answers, and errors, and builds the command
  and configuration JSON.
- `devices_controller_test.dart` covers each command, the refresh after it, the readable error
  messages, 2 s polling, and the five-minute discoverable window (with `fake_async`).
- `devices_screen_test.dart` covers the sections, the large and spoken codes, Accept and
  Codes match, the waiting state, the confirmed removal, make discoverable, and sync off.

`fake_p2p_bridge.dart` holds the scripted bridge and the `busyStatus` and `emptyStatus` fixtures.

`app_theme_test.dart` checks that both themes come from the same seed with the right brightness,
and that the app follows a light and a dark system setting.

`core_startup_test.dart` covers the status mapping, the waiting message, the keychain
explanation with Retry, and Quit.
