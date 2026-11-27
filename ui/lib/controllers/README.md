<!-- This directory contains the Dart-package recording state controller. -->

# Controllers

`RecordingController` maps bridge success or failure into displayable recording state. It contains
no `dart:ffi` code and can be exercised through an in-memory bridge fake. It asks
`MicrophonePermission` before each start or resume ("Microphone access is needed to record" when
refused) and shows "Unable to open the microphone" for status 7 (Issue #45). It shows every state
through `RecordingNotifier` and follows pause, resume, and stop made from the Android notification
without calling the bridge again (Issue #47).

`NetworkSettingsController` (Issue #64) reads and saves the relay and static peers for Settings,
and words the relay's state. `DevicesController` also makes and reads pairing invites (Issue #65).

`MeetingAttachmentsController` (Issue #54) picks several files and attaches them at once. It
reports `Attached N files`, plus how many could not be attached.

`PhotoCaptureController` owns one multi-shot camera session for a meeting:
- **Attached right away:** each photo is attached as soon as it is taken, so a photo taken during
  a recording gets that moment's meeting time from Rust.
- **Temporary files:** the camera's temporary file is deleted whatever happened, so no photo is
  left outside the app's storage.
- **Closing:** disposing the controller turns the camera off.

`MeetingExportController` runs one export at a time (Issue #48). On desktops it asks for a path
first (cancelling exports nothing; with no dialog available, such as Linux without zenity, the
core writes to its default export folder) and reports "Exported to <path>". On phones the core writes a
private file, the picker saves its bytes, and the private file is deleted whatever happens;
Markdown leaves attachments out there, because the picker saves a single file. On Android,
`shareMeeting` (the Export menu's "Share as …" entries, #114) shares instead: the share folder
`cache/shared/handoff` is emptied, the core writes the export there with its attachments, a
Markdown export's `<stem>_attachments/` files are moved up beside it (clashing names become
`name (2).ext`; the Markdown's relative links then no longer resolve, but every file travels),
and the share sheet opens with the files and the title.
`suggestedExportFileName` turns the title into a safe file name with the format's extension.

`LogSettingsController` holds Settings' diagnostics (Issue #46): it loads the core's level,
changes it (keeping the old one and explaining when the core refuses), and exports the logs. On
desktops the save dialog picks the path; on Android the core writes into `cache/shared`, after
the previous export there is deleted, and the share sheet opens.

`AttachmentController` owns selected-file workflow state and decodes the Rust-owned JSON array of
meeting candidates. It receives both a picker and `AttachmentBridge` interface, preserving
widget-test injection and ensuring no complex Rust struct crosses FFI.

`RecordingController.startRecording(title:)` normalizes a blank title to `null` and exposes the
active title. `MeetingListController` owns the meeting list, the selected meeting's detail, and
their loading and error states. One controller serves both the desktop list-detail layout and
the phone detail route, and it drops a detail response that arrives after a newer selection
(Issue #36).

`meeting_title_validation.dart` mirrors Rust's `MeetingTitle::parse`: it trims, then rejects
empty titles, titles over 200 Unicode characters, and control characters, with a message for each.
`MeetingListController.renameMeeting` sends the trimmed title and, on success, replaces the
summary in the list and the selected detail without reloading. It rethrows failures and reloads
the list when the meeting no longer exists (Issue #41).

`DevicesController` holds the P2P status and runs the pairing commands: make discoverable, pair,
confirm codes, cancel, accept, reject, and remove. It refreshes after each success and maps Rust
error codes to plain messages, keeping the last good status. It polls every 2 s while the Devices
destination is shown. `discoverable` is true for the five-minute pairing window, measured with
`package:clock`, so tests can fake time without leaving a timer pending (Issue #43).

`CoreStartupController` runs `startCore` and maps Rust's status to `ready`, `keychainUnavailable`
(status 6), or `failed`. A throwing start, such as a missing library, also counts as `failed`.
`start` can be called again to retry.

`AppLockController` (Android) decides when the app lock covers the app: at start-up and after
the app was out of sight for `lockAfterBackground` (30 s), while the setting is on and the device
has a screen lock. A failure to read the setting keeps the app locked. The prompt is shown once
per lock automatically, and again from Unlock. The app being hidden by the prompt's own PIN
screen does not lock it again. Turning the setting on or off needs a successful prompt first.
Only the UI is locked: the Rust core, recording, and sync keep running.

`transcription_route_controller.dart` (Issue #59) holds the Settings choice of where recordings are
transcribed; the core stores and applies it.

`transcription_settings_controller.dart` holds the Transcription page: models and downloads (#67); the OpenAI choice is gone (#117).

`summary_export_controller.dart` (#130) exports a meeting's summary as PDF, plain text, Word or
Markdown like a meeting export: the save dialog on desktops, a private copy handed to the system
picker and deleted on phones. The summary page shows it as an Export menu when
`SummariesController.summaryExport` is set.

`summaries_controller.dart` (#95) holds the AI settings and each meeting's summary for Settings,
the meeting lists and the summary page. It lives as long as the app, so a summary keeps being
written when its page is left; it ignores a second request for a meeting already being summarized
and never lets a slower read replace a summary or prompt written meanwhile.
`editSummary` saves the owner's own text through `MeetingEditBridge` (#96). On Android it also
carries the `HandoffController`, so every summary view can offer the owner's own AI app, and
`asksPairedComputer` is set, so `askComputer` and `cancelComputer` write or remove a request that
a paired computer write the summary (#107); `describePairedRequest` words its state. On a computer,
`setPairedSummaryConnection` chooses the connection that answers paired devices.

`call_watch_controller.dart` (#111) looks at the microphone's users every 3 seconds on desktops
(`offersToRecordCalls`: Linux and Windows). Android phones do not offer: Android gives a call the
microphone and other captures silence. `hostUsers` lets a host pass the users it sees itself.
- A call noticed while nothing is being recorded becomes `offeredApp`, shown as a banner and,
  where there is one, as a system notification (`CallNotifier`). Only `acceptOffer` (the owner's
  tap on either) starts a recording, titled "`<app>` call" ("Call" for Android's unnamed call app)
  and tied to the call.
- When the call ends, a tied recording stops if "Stop when the call ends" is on, and `message` says
  why. A recording the owner started themselves is never stopped by a call.
- Where the system cannot tell, the looks stop. A look that failed once does not stop them.

`handoff_controller.dart` (#114) hands a meeting to the owner's own Claude or ChatGPT app.

- **The hand-off:** the core writes the meeting and its files, the prompt also goes on the
  clipboard, and the app opens. `handOffNotes` (#126) does the same for a project's chat, or every
  note's: the meetings, the projects' files and every meeting's attachments. No answer is
  awaited.
- **The answer:** a summary hand-off waits up to an hour. When Koalino comes back, new
  clipboard text of at least 80 characters is kept as that meeting's summary. A `HandoffKept`
  event offers Undo, and `HandoffHint` says to copy the answer (or share it) first. An answer
  shared back to Koalino as text goes through `keepSharedAnswer`, with the same rules.
- **Chat:** a chat leaves nothing pending.

`meeting_edit_controller.dart` (#96) holds each meeting's notes as last read or saved, and saves
transcript corrections; `MeetingListController.replaceSegment` then shows the corrected (or
removed) passage in the open meeting without reloading it.

`app_language_controller.dart` (Issue #124) lists the ten app languages by their own names
(`appLanguages`) and holds the owner's choice: `null` follows the system. `load()` reads it at
start-up and `choose()` saves it through an `AppLanguageStore`.

`transcription_language_controller.dart` holds the default language and the language of the next or
current recording, and returns to the default when a recording ends (Issue #62).
`describeTranscriptionLanguages` names a meeting's languages for its detail screen.

`developer_settings_controller.dart` (#187) holds Settings → Developer: the seven taps on the
version row (taps more than 3 s apart start again) that unlock it, and the core's list of
overridable app variables. Saving, resetting and turning off go to the core, which validates them.

`projects_controller.dart` (Issue #85) holds the Projects tab: the project list, the open project
(meetings newest first, resources oldest first), and every change (create, rename, delete, add
files or shared paths, write a note, delete a resource, move a meeting). `projectNameError`
checks names with the rules of meeting titles. One controller lives as long as the app, so the
tab, meeting pages and files shared from other apps see the same state.
