<!-- This directory holds widget tests that define Flutter behavior before presentation implementation. -->

# Widget tests

Tests inject a `RecordingBridge` fake into the controller, proving that widgets make state changes
without loading a native Rust library or accessing Rust implementation types.

Meeting attachments (Issue #54) use `fake_photo_camera.dart` (a camera and an attachment bridge
without a device or Rust):
- `meeting_attachments_decoding_test.dart` covers:
  - the JSON contract;
  - where each attachment sits among the segments: after those that started before it, with
    untimed ones last.
- `photo_capture_test.dart` covers the multi-shot camera:
  - each photo is attached on its own, and its temporary file is deleted;
  - failures are reported;
  - the screen stays open for the next shot.
- `meeting_attachments_test.dart` covers the meeting detail:
  - photos and documents appear in place;
  - Attach files sends several picked files at once and reports failures;
  - Take photos opens the camera for this meeting and reloads it.
- `recording_screen_test.dart` covers Take photos while recording.

`whisper_model_test.dart` covers where the app finds its Whisper model: the data directory
first, then the bundle's `data/models` on desktops; Android installs the APK's copy.
`diarization_models_test.dart` covers the same for the two diarization models.

`transcription_language_test.dart` covers choosing the language in Settings and on the Record
screen, over `fake_language_bridge.dart`.

Meeting-title tests (Issue #36):

- `recording_screen_test.dart` covers the title field, its hint, sending a trimmed title or
  `null`, keyboard submit, locking while recording, and phone/desktop sizing. It also covers the
  microphone (Issue #45): a refused permission never reaches the bridge, and status 7 on start or
  resume shows "Unable to open the microphone". The notification group (Issue #47) checks the
  states shown to the notifier and that pause, resume, and stop from the notification update the
  screen.
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

`host_key_test.dart` covers `startCoreWithHostKey`: the desktop path without a host key, the
hand-over before start-up, a Keystore failure or refused key reported as status 6 without
starting the core, and the `note_taker/key_vault` method-channel request.

`app_lock_controller_test.dart` covers locking at start-up (on, off, no screen lock, a failing
status), unlocking (success, cancel, a double tap, one automatic prompt per lock), the 30 s grace
period, the prompt's PIN screen not relocking, and changing the setting.
`app_lock_screen_test.dart` covers the gate (nothing shows while checking, the automatic prompt,
Unlock after a cancel, app state kept across a lock, pushed routes covered, the lifecycle) and
the Settings switch. `fake_app_lock_bridge.dart` holds the scripted bridge.
`meeting_list_screen_test.dart` also covers Name speakers on phones and desktops.
`network_settings_test.dart`, `devices_network_test.dart`, and `network_routes_decoding_test.dart`
cover the relay and peer fields, the Devices screen's explanations, and invites.
`meeting_list_screen_test.dart` also covers deleting a meeting from the list and from an open
meeting, cancelling, and a refusal. `voice_gallery_screen_test.dart` covers marking the voice playing
and deleting unknown voices and saved people.

`voice_gallery_screen_test.dart` covers both the per-meeting and all-meetings gallery, and
`home_shell_test.dart` its top-level entry.

`meeting_export_test.dart` covers export (Issue #48): decoding the core's answer, safe file
names, the desktop save path, cancel, and the default-folder fallback, the phone save with the
private file removed (also on cancel), the failure messages, and the Export menu.

`log_settings_test.dart` covers Settings' diagnostics (Issue #46): the levels, loading and
changing the level (and a refusal), the desktop export, cancel, and its messages, the Android
share from the shared folder (an earlier export removed, a failing sheet), decoding the core's
answer, and the screen with and without App lock. `fake_log_bridge.dart` holds the scripted
bridge, sharer, and save dialog, which `app_lock_screen_test.dart` also uses.

`project_bridge_test.dart` covers the Projects bridge (Issue #85): requests, decoding a project,
an e-mail and an add-files answer, and refusals. `projects_screen_test.dart` drives the tab over
`FakeProjects`: creating with name validation, a project's meetings and resources, the e-mail and
document viewers, deleting a resource, moving a meeting from its page, the add-files message, and
the Markdown view. `fake_project_runner.dart` scripts `note_taker_project_command`.
