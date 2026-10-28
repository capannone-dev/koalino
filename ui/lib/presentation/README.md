<!-- This directory renders Material 3 recording controls from controller-provided state. -->

# Presentation

`RecordingScreen` observes `RecordingController` and calls its public methods. It has no knowledge
of Rust types, dynamic-library handles, or native status codes.

`AttachmentImportScreen` renders the Meeting Detail paperclip control and the Material 3 dialog
for Rust-ranked meeting suggestions. It depends only on `AttachmentController` and requires an
explicit user selection before the controller calls Rust to persist a link.

`HomeShell` switches between the Record, Meetings, and Telemetry destinations, with a bottom
`NavigationBar` below 840 px and a `NavigationRail` above it. `RecordingScreen` adds an optional,
width-capped meeting title field. `MeetingListScreen` renders titles above their date and
duration, and pushes `MeetingDetailScreen` on phones or shows `MeetingDetailView` beside the list
on desktops. The detail view puts the title in a bold `headlineMedium` semantic header
(Issue #36).

`rename_meeting_dialog.dart` holds `RenameMeetingDialog` and `showRenameMeetingDialog`, shared by
the list's context menu (long-press, right-click, Shift+F10, and a screen-reader action) and the
detail's pencil button. It validates locally, maps Rust failures to plain messages, and confirms
success with a snack bar (Issue #41).

`devices_screen.dart` shows this device, a "Make discoverable" button, incoming requests and
outgoing pairings with their six-digit codes in large type (read out digit by digit by screen
readers), paired devices with a confirmed Remove, and nearby devices with Pair. A disabled P2P
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

`settings_screen.dart` (`SettingsScreen`) is on every platform (Issue #46). Its diagnostics hold
the Log level selector (Error to Trace, Info by default; the core keeps the choice across
restarts) and Export logs (Share logs on Android), which saves or shares the last seven days of
logs. On Android it also holds the App lock switch, which explains why it is unavailable without
a screen lock or before Android 10. `HomeShell` shows Settings as a fifth destination when given
a builder, which the app always gives.

The meeting detail's app bar has Name speakers (`NameSpeakersButton`), which opens the voice
gallery (`voice_gallery_screen.dart`) for that meeting and reloads the meeting on return so the
transcript shows the new names. The desktop two-pane layout shows it once a meeting is selected.

Beside it, Export (`ExportMeetingButton`, Issue #48) offers Markdown, PDF, and JSON, shows a
spinner while exporting, and reports the result in a snack bar.
