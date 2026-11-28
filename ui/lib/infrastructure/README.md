<!-- This directory owns Dart's concrete boundary adapter to the Rust C ABI. -->

# FFI infrastructure

The recording bridge converts Dart calls to primitive C ABI functions and status codes. Rust
domain structs are never represented in Dart or presented to widgets.

`meeting_attachment_bridge.dart` (Issue #54):
- `PlatformAttachmentFilesPicker` wraps `file_picker` with multiple selection, limited to the
  types Rust accepts.
- `RustMeetingAttachmentBridge` calls `note_taker_attach_files` on a background isolate, since it
  copies files, and decodes the attached and failed files.

`photo_camera.dart` hides the `camera` plugin behind `PhotoCamera`:
- the back camera, without audio;
- `isCameraSupported` is false on Linux, which has no implementation;
- Windows uses the unendorsed `camera_windows`;
- Android declares `CAMERA`, asked at run time, and marks the camera feature optional.

`MeetingBridge.getMeeting` also decodes the meeting's `attachments`.

`developer_settings_bridge.dart` (#187) sends Settings → Developer's commands (`get`, `unlock`,
`set`, `reset`, `lock`) to `note_taker_developer_settings` on a background isolate and decodes the
list of overridable variables; a refusal becomes `DeveloperSettingsException` with the core's code
and its English explanation of the accepted values.

`speaker_rename_bridge.dart` (#186) decodes each segment's `voice` (a saved person or an unknown
voice) and defines `SpeakerRenamer`. `RustSpeakerRenamer` renames a person through
`note_taker_rename_person` and names an unknown voice through `note_taker_assign_cluster`, each on
a background isolate, and turns Rust's refusals into `SpeakerRenameException` codes.

`telemetry_bridge.dart` defines the telemetry DTOs and `RustTelemetryBridge`, which calls
`note_taker_get_devices`, `note_taker_get_telemetry_summary`, and
`note_taker_get_device_telemetry` on a background isolate and frees each response there.

Rule for every bridge: no native call runs on the UI isolate. Each worker isolate opens the
library and looks up its symbols itself, because `DynamicLibrary` handles and native pointers
can't cross isolates; only Dart strings and ints come back.

`RustRecordingBridge` binds start (also resume), pause, and stop, and runs each on a background
isolate. `startRecordingWithTitle` calls `note_taker_start_recording_with_title`,
sending a null pointer for the meeting ID and for a blank title. `meeting_bridge.dart` (whose segments carry `overlappingSpeech`, `uncertainSpeaker` and
`uncertainWords`, read as plain from an older core) defines the
`MeetingSummary`, `MeetingDetail`, and `MeetingTranscriptSegment` models, and the pure JSON
decoders with their fallback-title and formatting helpers. It also defines `RustMeetingBridge`,
which calls `note_taker_get_meetings` and `note_taker_get_meeting` on a background isolate and
frees each response with `note_taker_free_string` (Issue #36).

`RustMeetingBridge.renameMeeting` calls `note_taker_rename_meeting` on a background isolate, frees
both native strings, and maps non-zero statuses to `MeetingRenameException` through
`meetingRenameExceptionFor` (1 invalid title, 2 storage, 3 runtime, 5 not found, other unknown)
(Issue #41).

`recording_bridge.dart` maps start and resume statuses to `RecordingStartOutcome`: `started`
(0), `microphoneUnavailable` (7, the Rust core could not open the microphone), or `failed`.
`microphone_hearing.dart` asks the core (`note_taker_microphone_silence`, on a background isolate)
whether the recording's microphone is heard, or gives only silence as when it is muted, turned off,
or blocked by Windows' privacy settings; the Record screen then says so.

`microphone_permission.dart` asks for microphone access before each start or resume: Android
uses `MicrophonePermission.kt` on `note_taker/microphone` (`RECORD_AUDIO`); desktops have no
runtime prompt, and a Windows privacy refusal surfaces as status 7 (Issue #45).

`recording_notifier.dart` mirrors the recorder into Android's `RecordingService` and streams the
changes made from its notification (Issue #47); desktops use `NoRecordingNotifier`. The controller
starts the service before Rust opens the microphone, because Android refuses to start it once the
app has left the screen and then silences the microphone; `show` returns `false` when Android
refused it.

`whisper_model.dart` picks the model passed to the core, so a fresh install transcribes offline:
`<data directory>/models/ggml-base.bin` when the owner put one there, otherwise the bundle's
`data/models/ggml-base.bin` beside the executable (Linux, Windows). On Android, `WhisperModel.kt`
(channel `note_taker/whisper_model`) copies the APK's model into the data directory on first
start, or when an update ships a model of a different size.

`diarization_models.dart` does the same for the two diarization models: on desktops it passes the
bundle's `data/models/diarization` when both files are there (the core prefers a pair in the data
directory), and on Android it asks `WhisperModel.kt` to copy them into the data directory.

`transcription_settings_bridge.dart` reaches the core's Whisper model settings (#67); there is no
cloud transcription or key since #117. `setGlossary` saves the glossary (one term per line) and the
settings carry the saved terms. `setKeepSpeechDays` saves the speech retention window, which the
settings carry as `SpeechRetention` (`days` and its bounds, #93).

`network_state.dart` (#189) tells whether the network is unmetered: Android answers through
`note_taker/network` (`NetworkState.kt`); computers always count as unmetered.

`language_bridge.dart` reaches the core's default language and the language of the next or current
recording (`note_taker_get_default_language`, `note_taker_set_default_language`,
`note_taker_set_recording_language`) on background isolates (Issue #62).

`export_bridge.dart` exports a meeting through `note_taker_export_meeting` on a background
isolate and decodes the core's JSON answer into `MeetingExportSuccess` or `MeetingExportFailure`
(Issue #48). Formats are Markdown, PDF, and JSON. `FilePickerExportSaver` shows the save dialog:
on desktops it only picks a path (starting in Downloads) and the core writes there; on Android
and iOS the system document picker receives the bytes. `RustSummaryExportBridge` writes a
meeting's summary through `note_taker_export_summary` (#130) in a `SummaryExportFormat` (PDF,
text, Word, Markdown).

`log_bridge.dart` reaches the core's log level and log export (`note_taker_get_log_level`,
`note_taker_set_log_level`, `note_taker_export_logs`) on background isolates (Issue #46).
`LogLevel` lists the five levels Settings offers. `AndroidLogSharer` hands an exported file to
the share sheet on `note_taker/share` (`SharedFiles.kt`). `FilePickerExportSaver` takes the
dialog's title, so the log export says "Export logs".

`app_configuration.dart` runs first: `configureCore` passes the application-support directory
(`<support>/note_taker`), the device name (the host name; "Android device" on Android), and
`models/ggml-base.bin` when present to `note_taker_configure` on a background isolate. Every
platform uses its keychain. On Android, `host_key.dart` first asks Kotlin
(`AndroidKeyVault.provideEncryptionKey`, channel `note_taker/key_vault`) to unwrap the Keystore
key and hand it to Rust over JNI; no key bytes cross the channel. `startCoreWithHostKey` reports
any failure as status 6. On Android, `configureCore` also hands the configuration JSON and the key
directory to Kotlin (`background_core.dart`, channel `note_taker/background_core`), so the charging
job can open the core while the app is closed (#87); `BackgroundPassBridge` reads and requests
the battery exemption that job needs with Power saving on.
`appConfigurationJson` builds the JSON and is unit-tested; it also passes peer addresses compiled
in with `--dart-define=NOTE_TAKER_P2P_PEERS`, and a relay address with
`--dart-define=NOTE_TAKER_P2P_RELAY`, for development builds; addresses saved in Settings replace
both (Issue #64). After `runApp`, `startCore` calls
`note_taker_start_runtime` on an isolate, so sync starts with the app. `CoreStartupController`
holds the screens back until it returns 0. Status 6 means the keychain is missing, locked, or
refused.

`p2p_bridge.dart` defines `P2pStatus` and its rows (with `DeviceCapabilities` for this device and
each connected paired device), `P2pCommandResult`, and `P2pException` (with
Rust's `error_code`), the pure decoders `decodeP2pStatus` and `decodeP2pCommandResult`, and
`p2pCommandJson`. `RustP2pBridge` calls `note_taker_p2p_status` and `note_taker_p2p_command` on
background isolates; a command blocks its isolate until the P2P task answers (Issue #43).

`app_language_store.dart` (Issue #124) keeps the chosen app language in the `ui-language` file
in the app's data directory (`FileAppLanguageStore`). A missing or empty file means "follow the
system".

`app_lock_bridge.dart` defines `AppLockStatus` (supported, device secure, enabled) and the
`AppLockBridge` interface. `AndroidAppLockBridge` calls Kotlin's `AppLock` on the
`note_taker/app_lock` channel: `status`, `authenticate` (the system prompt; false when
cancelled), and `setEnabled`. There is no desktop implementation.

`project_bridge.dart` (Issue #85) defines `ProjectSummary`, `ProjectDetail`, `ProjectResource`
(with its `ProjectResourceKind`: image, markdown, pdf, email, text, document), `EmailView`, and
`ProjectCommandException` (Rust's `error_code`). `ProjectBridge` sends one JSON command per call
through `note_taker_project_command` on a background isolate (`runRustProjectCommand`); tests
pass a fake runner. `PlatformProjectFilesPicker` picks any file (#94).

`llm_bridge.dart` (#95) defines `LlmConnection`, `LlmSettings`, `StoredSummary`,
`MeetingSummaryState` (with `PairedSummaryRequest`, a request to a paired computer, #107),
`SummaryResult`, the `llmPresets` (a local Ollama or LM Studio
server; online services with an API key, each with a one-line description, #138; and on desktops
Claude Code and Codex, #105, through `availableLlmPresets`) and
`LlmException`, which words the core's `error_code` for people and, for a rejection or a failed
local tool, adds what the server or tool said (`error_detail`, #104, #105). `LlmBridge`
calls `note_taker_llm_command`, `note_taker_summarize_meeting`, `note_taker_get_meeting_summary`,
`note_taker_set_meeting_summary_prompt` and `note_taker_request_paired_summary` (#107) on a
background isolate (`callRustLlm`), since a
summary waits on the model for minutes; tests pass a fake call. API keys are sent once and never
read back.

`recordings_bridge.dart` (#140) calls `note_taker_recordings_command`: `status`, `setKeep`,
`wipe`, `has`, `prepare` (the decrypted WAV's path and length), `export` (the same WAV written at
a path the app picks, for sharing or saving; #199) and `release`.

`recording_playback.dart` (#197) is `RecordingPlayback`, the small interface the recording player
plays through (open, resume, pause, seek, speed, position events), and `AudioPlayersPlayback`, its
`audioplayers` implementation. Tests swap in a fake that plays nothing.

`backup_bridge.dart` (#167) is `BackupBridge` and, on Android, `AndroidBackupBridge` over
`note_taker/backup` (`BackupChannel.kt`): document pickers, `create` and `stage`, and `restart`.
The database key never passes through Dart. `restore_on_start.dart` runs first in
`configureCore`: a staged restore with its ready marker replaces `note_taker`, the old folder is
kept as `note_taker.before-restore` (with its models moved into the restored folder, since a
backup leaves them out), and an unfinished staging folder is deleted.

`legacy_data_migration.dart` (#168) runs before that on desktops: when the permanent app ID's
folder has no `note_taker` data yet, the most recently used preview folder's contents (Linux
`com.example.note_taker_ui`, macOS `com.example.noteTakerUi`, Windows `com.example\Koalino` or
`com.example\note_taker_ui`) move into it, and a `MOVED-TO-KOALINO.txt` note is left behind.

`meeting_edit_bridge.dart` (#96) is `MeetingEditBridge`: the meeting's notes, the summary's text
and single transcript segments, through `note_taker_meeting_edit` (one JSON request per call, on
a background isolate through `callRustLlm`). `MeetingEditException` words the core's `error_code`
(`empty`, `too_long`, `segment_not_found`, ...). Note text is never logged.

`search_bridge.dart` (#100) is `SearchBridge.search(query, scope, projectId:)` over
`note_taker_search` (on a background isolate through `callRustLlm`), with `SearchHit` (where the
match is: `placeLabel` such as `1:01 · Anna`, and the snippet's `SnippetPart`s),
`SearchResults` and `SearchException`, which words the core's `error_code`.

`chat_bridge.dart` (#97) is `ChatBridge.history(topic)`, `ask(topic, question)` and
`clear(topic)` over `note_taker_chat` (on a background isolate through `callRustLlm`), with
`ChatTopic` (a meeting, a project or everything), `ChatEntry` with its cited `ChatSource`s
(#110) and `ChatException`. On the phone (`canAskPairedComputer`), `ask(..., onPairedComputer:
true)` has a paired computer answer from its own copy of the notes (#107).

`handoff_bridge.dart` (#114) holds the two halves of handing a meeting to the owner's own AI app.
`RustHandoffCore` calls `note_taker_handoff_meeting` and the `keep_answer` / `restore_summary`
edits. `AndroidHandoffPlatform` covers the folder (`cache/shared/handoff`), the installed apps,
`shareFiles` and the clipboard. `clearHandoffFolder` deletes an earlier hand-off's copies at
start-up. `RustHandoffCore(inline: true)` asks for a message that carries the meeting itself.
`prepareNotes` (#126) asks for a project's chat hand-off (`project_id`), or every note's
(`all`). `PreparedHandoff.hasFilesToDrag` is true when files, or the Markdown itself
(`documentAttached`), must be dragged into a website.

`desktop_handoff.dart` (#126) is the computer's `HandoffPlatform`, for owners who use claude.ai or
chatgpt.com rather than Claude Code or Codex. Both websites are always offered (`viaBrowser`).
`share` opens the website with `openWithSystem` (`xdg-open`, or Explorer's URL handler on
Windows). A message of up to 12,000 characters goes in the address (`AiApp.prefilledAddress`,
`?q=`): claude.ai fills its message box, and chatgpt.com also sends it. `prefills` tells the
controller which, so the steps dialog (`showHandoffSteps`) says "press Enter", "it sends by
itself" or "paste with Ctrl+V". It also opens the hand-off folder (`<support>/handoff`) when the meeting has photos or
files to drag in. With files, chatgpt.com is not filled through its address: it would send the
message before the files are in. The controller has already put the message on the clipboard, and it reads the
answer back when the window is active again.

`clear_copy_bridge.dart` (#99) gives the viewers clear copies of sealed attachments:
`clearCopyResolver` (replaceable in tests) defaults to `rememberedClearCopy`, which calls
`note_taker_clear_copy` on a background isolate and reuses a copy for 20 minutes (the core keeps
it 30). Failures carry the core's code in `ClearCopyException`. The `filePath` of attachments and
resources names the sealed file, which only the core can read.

`notes_folder_bridge.dart` (#98) is `NotesFolderBridge.status()`, `set(enabled:, path:)` and
`syncNow()` over `note_taker_notes_folder` (on a background isolate through `callRustLlm`, since a
round writes the whole folder), with `NotesFolderStatus` and `NotesFolderException`.

`assistant_access_bridge.dart` (#112) is `AssistantAccessBridge.status()` and `set(enabled:)` over
`note_taker_assistant_access`, with `AssistantAccessStatus` (the switch, the installed server and
the snippets to connect Claude Code or another MCP client) and `AssistantAccessException`.

`call_watch_bridge.dart` (#111) is `CallWatchBridge.poll(users:)`, `settings()` and
`set(offer:, autoStop:)` over `note_taker_call_watch`, with `CallPoll` and `CallWatchSettings`.
`call_notifier.dart` shows the same offer as a system notification, so it is seen while the
window is hidden. On Linux, `NotifySendCallNotifier` runs `notify-send` with Record and Not now
buttons (libnotify 0.7.9 or later prints the chosen one). It is withdrawn when the offer is
answered in the app or the call ends. On Windows, `CoreToastCallNotifier` asks the core for a
toast (`note_taker_call_toast`; Windows PowerShell cannot receive a toast's button events).
Other platforms use `NoCallNotifier` (the banner only). No
plugin is needed. When no notification can be shown (`NoCallNotifier`, or `notify-send` missing
or failing), the answer is `unavailable`, which leaves the banner up; only `dismissed` clears it.
`android_call_signals.dart` is the Android side: `AndroidCallSignals.silenced()` asks Kotlin
(`note_taker/call_signals`) whether a call is making Android feed the recording silence. The
phone does not offer to record calls (`offersToRecordCalls`, #111).

`incoming_share_bridge.dart` (Issue #85) hands over what other apps share to Koalino:
`IncomingShare` (the copies' folder, their paths, and how many could not be read) and
`IncomingShareBridge` (`arrivals`, `takePending`, `release`). `AndroidIncomingShareBridge` talks
to `IncomingShares.kt` on `note_taker/incoming_shares`; desktops get `NoIncomingShareBridge`.

`pairing_link_bridge.dart` (Issue #122) hands over the pairing invite links a phone's camera opens
from another device's QR code: `PairingLinkBridge` (`arrivals`, `takePending`, which returns only
`note-taker-invite:` text). `AndroidPairingLinkBridge` talks to `PairingLinks.kt` on
`note_taker/pairing_links`; desktops get `NoPairingLinkBridge`.
