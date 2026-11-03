<!-- This directory renders Material 3 recording controls from controller-provided state. -->

# Presentation

`RecordingScreen` observes `RecordingController` and calls its public methods. It has no knowledge
of Rust types, dynamic-library handles, or native status codes.

Meeting attachments (Issue #54):
- **Meeting detail and the desktop list-detail app bar:**
  - Take photos (`TakePhotosButton`, when a camera exists) and Attach files (`AttachFilesButton`)
    act on the meeting being viewed and reload it afterwards.
  - The transcript places each attachment with `buildMeetingTimeline`: a photo shows as a
    picture (tap for full screen), a document as a card, and anything added after the meeting
    under "Added after the meeting".
- **Record screen:** offers Take photos (where a camera exists) and Attach files while
  recording or paused. They act on the meeting Rust reports as active, so what they add lands
  at that point of the transcript; on Linux, Attach files is the only way.
- **`PhotoCaptureScreen`:**
  - the shutter can be pressed again and again;
  - thumbnails of the photos taken so far;
  - Done.

`pairing_invite_dialogs.dart` shows an invite as a QR code and a copyable link, and takes one pasted
from another device (Issue #65). `devices_screen.dart` offers both, and explains why devices on other
networks are not found, depending on whether a relay is set (Issue #66). Settings has the relay and
static peer fields, which apply at once (`NetworkAddressesSection`, Issue #64).

`HomeShell` switches between the Record, Meetings, Voices and Settings destinations (Devices is a page of
Settings, Issue #84), with a bottom
`NavigationBar` below 840 px and a `NavigationRail` above it. `RecordingScreen` adds an optional,
width-capped meeting title field. `MeetingListScreen` renders titles above their date and
duration, and pushes `MeetingDetailScreen` on phones or shows `MeetingDetailView` beside the list
on desktops. The detail view puts the title in a bold `headlineMedium` semantic header
(Issue #36).

`confirm_delete_dialog.dart` asks before any irreversible deletion, and `delete_meeting_action.dart`
uses it for the meeting list and detail, reporting a refusal in a snack bar.

`rename_meeting_dialog.dart` holds `RenameMeetingDialog` and `showRenameMeetingDialog`, shared by
the list's context menu (long-press, right-click, Shift+F10, and a screen-reader action) and the
detail's pencil button. It validates locally, maps Rust failures to plain messages, and confirms
success with a snack bar (Issue #41).

`devices_screen.dart` shows this device, a "Make discoverable" button, incoming requests and
outgoing pairings with their six-digit codes in large type (read out digit by digit by screen
readers), paired devices with a confirmed Remove, and nearby devices with Pair. This device and
each paired device show their manifest in one line (`describeCapabilities`, such as
"8 CPU cores · Whisper (on device)"), or "Not connected". A disabled P2P
task shows "Device sync is off". `HomeShell` lists it as the fourth destination, Devices
(Issue #43).

`app_theme.dart` builds the light and dark Material 3 themes from one indigo seed with
`noteTakerTheme(brightness)`. `NoteTakerApp` sets both and `ThemeMode.system`, so the app follows
the system setting, including changes while it runs. Screens take colours from the theme's
`colorScheme`, so both themes keep their contrast.

`core_startup_gate.dart` (`CoreStartupGate`) shows "Unlocking your notes…" while the core opens,
which may wait on a keychain unlock prompt. On a keychain or storage failure it explains the
problem and offers Retry and Quit, instead of the app. There is no key-file fallback
(`DECISION_LOG.md`).

`app_lock_gate.dart` (`AppLockGate`, Android) wraps the navigator through `MaterialApp.builder`,
so pushed screens and dialogs are covered too. While locked it shows "Note Taker is locked" with
Unlock; the app's widgets stay mounted but hidden (not painted, tapped, or read by screen
readers), so an active recording and every screen keep their state. It forwards the app being
hidden and shown to `AppLockController`.

`transcription_settings_screen.dart` lists the Whisper models by size, and holds the OpenAI choice and write-only
key field (#67).

`settings_screen.dart` (`SettingsScreen`) is on every platform (Issue #46). Its diagnostics hold
a Telemetry entry, which pushes the telemetry dashboard as a page (Issue #53; it used to be a
destination), the Log level selector (Error to Trace, Info by default; the core keeps the choice across
restarts) and Export logs (Share logs on Android), which saves or shares the last seven days of
logs. On Android it also holds the App lock switch, which explains why it is unavailable without
a screen lock or before Android 10. `HomeShell` shows Settings as the fourth destination when given
a builder, which the app always gives.

The meeting detail's app bar has Name speakers (`NameSpeakersButton`), which opens the voice
gallery (`voice_gallery_screen.dart`) for that meeting and reloads the meeting on return so the
transcript shows the new names. The desktop two-pane layout shows it once a meeting is selected.
The Voices destination opens the same screen for all meetings, with each voice's meeting and the
Saved people section (Issue #63). The name field's menu picks a saved person, and Rust writes a
clip to a file only when Play is pressed.

Beside it, Export (`ExportMeetingButton`, Issue #48) offers Markdown, PDF, and JSON, shows a
spinner while exporting, and reports the result in a snack bar.
