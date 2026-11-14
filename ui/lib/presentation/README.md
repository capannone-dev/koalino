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
  - Each segment shows how far it can be trusted (#90, #91): a guessed speaker reads
    `Name (?)`, overlapping speech gets an "Overlapping speech" label, and unclear words are in
    italics in the outline colour.
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

A meeting recorded on several devices (#88), such as the laptop and a phone near the other person,
is one meeting: its list row and detail say "Recorded on Laptop + Pixel". The detail's Split
button (`split_meeting_action.dart`) asks first, then lists the other devices' recordings on their
own again, for good.

`rename_meeting_dialog.dart` holds `RenameMeetingDialog` and `showRenameMeetingDialog`, shared by
the list's context menu (long-press, right-click, Shift+F10, and a screen-reader action) and the
detail's pencil button. It validates locally, maps Rust failures to plain messages, and confirms
success with a snack bar (Issue #41).

`devices_screen.dart` shows this device, a "Make discoverable" button, incoming requests and
outgoing pairings with their six-digit codes in large type (read out digit by digit by screen
readers), paired devices with a confirmed Remove, and nearby devices with Pair. This device and
each paired device show their manifest in one line (`describeCapabilities`, such as
"8 CPU cores · Whisper (on device)"), or "Not connected". A disabled P2P
task shows "Device sync is off" (Issue #43). It opens from Settings (Issue #84).

`app_theme.dart` builds the light and dark Material 3 themes from one indigo seed with
`noteTakerTheme(brightness)`. `NoteTakerApp` sets both and `ThemeMode.system`, so the app follows
the system setting, including changes while it runs. Screens take colours from the theme's
`colorScheme`, so both themes keep their contrast.

`core_startup_gate.dart` (`CoreStartupGate`) shows "Unlocking your notes…" while the core opens,
which may wait on a keychain unlock prompt. On a keychain or storage failure it explains the
problem and offers Retry and Quit, instead of the app. There is no key-file fallback
(`DECISION_LOG.md`).

`app_lock_gate.dart` (`AppLockGate`, Android) wraps the navigator through `MaterialApp.builder`,
so pushed screens and dialogs are covered too. While locked it shows "Koalino is locked" with
Unlock; the app's widgets stay mounted but hidden (not painted, tapped, or read by screen
readers), so an active recording and every screen keep their state. It forwards the app being
hidden and shown to `AppLockController`.

`transcription_settings_screen.dart` lists the Whisper models by size, with no cloud provider
or key field (#67, #117). The first row, "Automatic" (the default, #106), names the model it runs here and why,
with a Download button while the suggested model isn't on the device; picking a model by hand leaves it, and
the efficient mode switch shows only then. Its Glossary section takes one name or term per line; the message under Save says
it applies from the next recording, and refused text stays in the field to correct. Below it,
"Keep speech for combining" is a menu of days (1 to 60, plus any window set from the CLI or the
web) that `note_taker_set_keep_speech_days` saves (#93). Above it, the "Improve the transcript after
the meeting" switch (on by default) is saved by `note_taker_set_after_meeting_pass` (#87); while it is on, "Model for this pass" offers "Same as for recording" or a model already on the device (`note_taker_set_after_meeting_pass_model`). On a desktop build that can, "Also record the computer's sound" (on by default) is saved by `note_taker_set_computer_sound` (#108); the row is hidden where the core reports it unavailable.

`settings_screen.dart` (`SettingsScreen`) is on every platform (Issue #46). Its diagnostics hold
a Telemetry entry, which pushes the telemetry dashboard as a page (Issue #53; it used to be a
destination), the Log level selector (Error to Trace, Info by default; the core keeps the choice across
restarts) and Export logs (Share logs on Android), which saves or shares the last seven days of
logs. On Android it also holds the App lock switch, which explains why it is unavailable without
a screen lock or before Android 10. `HomeShell` shows Settings as the last destination when given
a builder, which the app always gives. Its last row, About Koalino (`about_tile.dart`, #115), opens
the About dialog: the version, the GPL-3.0-or-later notice, and View licenses for the Flutter
packages.

The meeting detail's app bar has Name speakers (`NameSpeakersButton`), which opens the voice
gallery (`voice_gallery_screen.dart`) for that meeting and reloads the meeting on return so the
transcript shows the new names. The desktop two-pane layout shows it once a meeting is selected.
The Voices destination opens the same screen for all meetings, with each voice's meeting and the
Saved people section (Issue #63). The name field's menu picks a saved person, and Rust writes a
clip to a file only when Play is pressed.

Beside it, Export (`ExportMeetingButton`, Issue #48) offers Markdown, PDF, and JSON, shows a
spinner while exporting, and reports the result in a snack bar.

Projects (Issue #85) is the destination between Meetings and Voices:

- `projects_screen.dart` lists projects by name with their counts; New project asks for a name.
- `project_detail_screen.dart` shows a project's meetings and its files, notes and e-mails, with
  Add files, Write a note, Add meetings, Rename and Delete. A meeting opens its usual page;
  removing it from the project keeps the meeting. A file still syncing says so.
- `resource_viewer_screen.dart` shows every resource inside the app: pictures zoomable, Markdown
  through `markdown_view.dart`, PDFs with `pdfrx`, e-mails as headers and plain text (Rust turns
  HTML into text), and text files. Office documents say they can't be shown yet.
- `markdown_view.dart` renders Markdown with the pure-Dart `markdown` parser. Links are inert and
  images show their description, so nothing reaches the network.
- `meeting_summary_screen.dart` (#95) is a meeting's summary page: the stored summary (Markdown,
  shown with `MarkdownView(embedded: true)`), "Summarize with" a connection, and the meeting's own
  prompt. `MeetingSummaryButton` (the summary icon beside each meeting in the meeting list, a
  project's meetings, and an open meeting's app bar) opens it and spins while a summary is
  written; `showMeetingSummaryMenu` (row long press or right-click) offers Summarize with ▸ each
  connection, the meeting's prompt, and a return to the Settings prompt.
  The page also offers Edit summary, or Write a summary when there is none (#96); a summary
  written by hand reads "Written by you". On a phone, the page and the menu offer "Summarize on
  my computer" (#107): a paired computer writes it with its own connection. The page then shows
  whether the request waits, is being written (with a progress bar) or failed (with the reason
  and Try again), offers Cancel request, and reads the request again every 10 s while it is open.
- `meeting_edit_widgets.dart` (#96) holds the owner's editing tools: `MarkdownEditorScreen`
  (opened by `showMarkdownEditor`, with Edit / Preview and a Save action), `MeetingNotesSection`
  (the Notes section of an open meeting, read again each time a meeting is shown), and
  `EditableSegment`, which opens a dialog to correct a transcript passage on a long press, a
  right-click or the screen reader's "Edit text" action; leaving the text empty removes it.
- `search_screen.dart` (#100) is the search page, opened from the search button of the Meetings
  screen (every meeting), the Projects screen (every project) and a project's page (that
  project). Each result names its meeting or project and where the match is, and marks the
  matched words. A meeting result opens the meeting scrolled to the passage
  (`MeetingDetailScreen.focusStartMs`, which marks it); a file result opens the file
  (`openProjectSearchHit`).
- `clear_file.dart` (#99) is `ClearFile`: it asks for an attachment's clear copy, shows a progress
  indicator meanwhile, then builds the view from the copy's path, or an "unavailable" view. Meeting
  photos (inline and full screen) and project pictures, Markdown, text and PDFs use it.
- `chat_screen.dart` (#97) is the chat page (`openChat`): the kept messages, a question field,
  and under each answer what the model was given, what was left out and which connection
  answered. The places an answer cites are chips under it (#110): a meeting opens at the cited
  moment, a file in its viewer. It opens from the Meetings screen (every note), a meeting (its
  app bar, or the two-pane app bar for the selected meeting) and a project's page. On the phone a
  paired computer answers by default (#107); the "Who answers" menu switches to the phone's own
  AI connection.
- `notes_folder_screen.dart` (#98) is Settings → Notes folder on desktops: the folder (typed or
  chosen with the system dialog), the switch, Sync now, the last round, and the warning that the
  folder is a decrypted copy.
- `assistant_access_screen.dart` (#112) is Settings → AI assistant access on desktops: the switch
  that lets the owner's own assistant read the notes through the read-only MCP server, the warning
  that the assistant may send what it reads to its online model, and, once on, the `claude mcp
  add` line and the `mcpServers` settings, each with a copy button.
- `call_offer_banner.dart` (#111) puts "`<app>` is using the microphone. Record this call?" with
  Not now and Record above every screen on desktops. It shows a snackbar when a call's recording
  stopped because the call ended. `CallWatchSettingsTiles` are the two Settings switches, "Offer
  to record calls" and "Stop when the call ends", shown where the system can tell.
- `ai_settings_screen.dart` (#95) is Settings → AI connections: add (from a preset), edit, test,
  remove and choose the default connection; the summary prompt template with its placeholders;
  and automatic summaries (#101). Keys are typed in a hidden field and never shown again. On
  desktops it also offers Claude Code and Codex (#105), which need no address or key and show
  whether the tool is installed. On a computer, "Summaries for my other devices" chooses the
  connection that writes the summaries paired phones ask for, or Off (#107).
- `project_dialogs.dart` holds the name, note and meeting-picker dialogs, and
  `MoveToProjectButton`, which meeting pages show to move a meeting into a project or out.
  `chooseProjectForMeeting` is its chooser, ending in "New project…", which names, creates and
  moves in one step; the meeting list's row menu offers it as "Add to project…" (#94).
- `handoff_widgets.dart` (#114, Android) holds `showHandoffSheet`: Summarize or Chat in the
  installed Claude or ChatGPT app, or in another app through the share sheet. The summary menu
  and the summary page offer it as "Use your own AI app". It also holds `HandoffReturnListener`,
  which shows "Summary from Claude saved", with Undo, and the hint to copy the answer.
- `share_to_project.dart` (`IncomingShareListener`, Android) shows "Add the shared file to a
  project" when another app shares files or an e-mail: New project, or one of the projects. It
  sits inside the start-up gate, so a share that launched the app waits for the core. The
  platform's copies are released whether the files were added or the sheet dismissed. While a
  summary hand-off waits (#114), shared text is first offered as 'Save as the summary of
  "<title>"' or 'Add to a project instead'.
