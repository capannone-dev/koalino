<!-- This directory contains executable composition roots that wire ports to adapters at runtime. -->

# Binary entry points

`cli.rs` is the command-line composition root and `daemon.rs` is the headless service composition root. They own process-level configuration such as logging. `ffi.rs` owns the C-compatible Flutter boundary: it exposes primitive status-code functions only, keeping Rust domain structs and adapter implementations out of Dart.

`device_identity.rs` resolves the acquisition device ID that the CLI, daemon, and FFI stamp on
meetings, attachments, and log lines (requirement 3.3): `NOTE_TAKER_DEVICE_ID` when set, otherwise
a `device-<uuid>` generated once and kept in `<database>.device-id` beside the database. A blank or
corrupted ID file stops start-up instead of being replaced, because a new ID would split one
device's notes across two identities.

Attachment import uses a caller-freed JSON string for ranked metadata only. `ffi.rs` composes
`AttachmentManager` with the local metadata and SQLite adapters, then receives opaque IDs back for
the user-confirmed persistence action; it never accepts attachment bytes from Flutter.

For background services and lock-screen controls, `ffi.rs` owns its recorder in a process-wide
`OnceLock<Mutex<...>>`; no FFI caller receives or retains a Rust pointer. Start, Pause, Stop,
interruption, and state-query commands take only this mutex and operate on the portable
`AudioCapture` state machine. State query codes are deliberately `10` through `15`, leaving the
documented nonzero status codes distinguishable from lifecycle states.

## Live recordings (Issue #32)

- `pipeline_composition.rs` is the only module that names concrete AI adapters. `PipelineConfig`
  reads `NOTE_TAKER_WHISPER_MODEL`,
  `NOTE_TAKER_WHISPER_LANGUAGE` (`auto` by default; whisper.cpp would otherwise force English;
  an unknown code stops start-up; recordings without a chosen language carry it, and the daemon's
  Teams meetings record it),
  `NOTE_TAKER_ENABLE_DIARIZATION`,
  `NOTE_TAKER_DIARIZATION_SEGMENTATION_MODEL`, `NOTE_TAKER_DIARIZATION_EMBEDDING_MODEL`, and
  `NOTE_TAKER_VAD_ENABLED`. Diarization requested without both models, or in a build without the
  `diarization` feature, logs why and leaves segments unattributed; named models that cannot load
  are an error.
  Transcription is always local (whisper.cpp); the OpenAI provider was removed (#117).
  `spawn_recording_controller` builds the pipeline on the
  worker thread, with its own SQLite connections sharing the verified cipher and the encrypted
  voice library as matcher. No diarization adapter exists yet, so segments are `Unattributed`.
  Each transcribed chunk saves a telemetry row on a separate connection (Issue #53), so the
  Telemetry dashboard has data. Local whisper is recorded as `transcription` with the model's file
  stem at an estimated 5 W.
- `recording_controller.rs` (`RecordingController`) drives one recording: PCM → `LocalRecorder`
  (lifecycle; audio is dropped while paused) → `VadChunker` → `ChunkAudioStore` plus a
  `VadAudioChunk` with meeting-relative offsets → `RecordingManager`. A recording into an existing
  meeting continues after its last segment. Interruption flushes buffered audio. Stop waits for
  every chunk and wipes raw audio that no engine consumed.
  `with_screen_share(monitor, interval)` (Issue #49) gives every recording a
  `ScreenShareWatcher`.
- `speed_check.rs` (#109) times the clip in `core/assets` with Automatic's model, `CHECK_RUNS`
  times, saving each run to telemetry so the #68 rule counts the model as measured, and a smaller
  model when it falls behind. Results go to `<database>.speed-check.json`. The worker runs it after
  its rounds when a pass could run (no recording; a phone charging) and it stops between runs
  when that changes. When a check is due (`speed_check::is_due`), the runtime builds the
  pipeline, and with it the worker, at launch on a desktop and on charging on a phone.
  `ffi_transcription::speed_check_target` reads what to time, and Settings
  shows the result as `automatic.speed_check`. `speed_check_tests.rs` and
  `ffi_transcription_tests.rs` specify them.
- `refinement_worker.rs` (#87, #88) runs the after-meeting work on its `transcript-pass` thread:
  each round folds recordings of one meeting made here and on a paired device, then carries out
  each retained meeting's plan (send, combine, pass alone, delete). Folding, sending and deleting
  need no charging; passes do on a phone. A device with P2P but no pass model still runs it
  (`RefinementWorker::without_pass`) to hand its speech to the combining device.
  `RetainedSpeechInbox` stores speech paired devices send, only for meetings this device combines
  and only from the device that recorded it. `pipeline_composition::start_transcript_pass`
  connects both to the P2P task (`SpeechLink`, implemented by `P2pHandle`).
  Originally (#87) it ran only the pass. Both passes go through `resumable_pass_engine.rs`,
  which keeps each finished window's transcript, sealed, in the meeting's progress file
  (`RetainedAudioStore::finished_window`), so a pass a phone's background job had to stop
  resumes instead of starting over.
  `RefinementControl` says when it may run: no recording, not shutting down, and on a phone only
  while charging. The controller holds it while recording (`with_transcript_pass`) and queues
  each speech chunk on the `RetainedSpeechWriter`, which compresses it and appends it to the
  `RetainedAudioStore` off the capture path; Stop flushes the writer before releasing the pass. After Stop, and at start for leftover meetings,
  `RefinementWorker` loads the pass model (`NOTE_TAKER_TRANSCRIPT_PASS_MODEL`, else the live one),
  runs `refine_meeting`, and drops the model. Audio is deleted after a pass, when unreadable,
  when the meeting was deleted, or after three failures. `record start` waits for the pass after
  Stop. The pass is on by default except on Android, which needs a charging job first.
- `kept_recordings.rs` (#140) is the owner's opt-in "Keep meeting recordings": the switch
  (`keep_recordings` in `<db>.transcription-settings.json`, per device, off by default), the
  `recordings` folder the worker moves sealed speech into instead of deleting it
  (`RetainedAudioStore::keep_in`), playback as a WAV with each chunk at its time (a folded
  recording shifted onto its meeting), the same WAV written where the owner shares or saves it
  (`export`, #199), usage and Wipe all. `ffi_recordings.rs` exposes it to the
  app as `note_taker_recordings_command`; deleting a meeting deletes its recording.
- `screen_share_watcher.rs` runs `ScreenShareMonitor::sample` on its own `screen-share-watcher`
  thread at that interval, so frames never touch the audio path:
  - it attaches new slides to the recorded meeting at the meeting time the controller publishes
    through `RecordingClock`;
  - it captures nothing while paused or interrupted, and stops before Stop flushes the audio;
  - it hands the monitor back for the next recording;
  - failures are logged without screen contents.

  No composition enables it yet, because no conferencing-app `ScreenShareDetector` exists.
  `NoScreenShareDetector` and `NoScreenCapturer` (`adapters/media`) stand in for one. An app
  adapter plugs in through `ScreenShareMonitor::new(detector, capturer, SlideAttachmentStore)`,
  sampled every 2 s.
- `ffi.rs` composes the controller on the first `note_taker_start_recording` and reuses it for
  every later recording. A failure returns `FFI_PIPELINE_UNAVAILABLE` (`4`) and is retried on the
  next start. `note_taker_start_recording_for_meeting`,
  `note_taker_start_recording_with_title` (Issue #35), `note_taker_push_audio_pcm`, and
  `note_taker_get_active_meeting_id` complete the ABI (see `API_SPECS.md`).
  `note_taker_attach_files` (Issue #54) copies pictures and documents into
  `<data_directory>/attachments/` through `AttachmentFileStore`. While the meeting is being
  recorded, it stamps them with `RecordingController::current_meeting_time_ms`.
  With the `microphone` feature, start and resume open the default microphone first and return
  `FFI_MICROPHONE_UNAVAILABLE` (`7`) without starting when it cannot open; pause, interruption,
  and stop close it and push its queued tail. A `microphone-drain` thread moves captured audio
  into the recording every 100 ms, taking the runtime lock only to drain (Issue #45).
  `RecordingController::start_titled` stores the title through the worker's repository once the
  session is open. Stop blocks while the
  last chunks are transcribed, so Flutter must call it from a background isolate.
- `cli_recording.rs` runs `record start [--input <wav|->] [--title <title>] [--language <code>]` in the foreground until the input ends, on
  Ctrl+C, or on `record stop`. Without `--input` it records the default microphone (builds with
  the `microphone` feature; others ask for `--input`), and a stop transcribes the audio the
  microphone still held. Processes coordinate through `<db>.recording-active` (PID and
  meeting) and `<db>.recording-stop`. A lock left by a dead process is replaced.
- `daemon.rs` composes the pipeline at start-up on a blocking thread and logs its availability (a
  missing engine is not fatal). On SIGINT or SIGTERM it shuts down gracefully, letting the worker
  drain.

## P2P pairing and sync (Issue #43)

`transcription_host.rs` runs paired devices' transcriptions on its own thread (#59), loading the
engine on the first job and deleting each job's audio when it ends. `paired_chat_host.rs` answers
paired devices' chat questions on its own thread (#107), one at a time, with a queue of four,
declining at once when the owner chose no connection for paired devices (`not_offered`) and
beyond 30 questions an hour (`rate_limited`). The app (`ffi_chat.rs`) and the daemon
(`server/chat_routes.rs`) attach it to the P2P task with `P2pHandle::answer_paired_chats_with`.

`p2p_composition.rs` runs the daemon's P2P task on its own `note-taker-p2p` thread, with a
single-threaded Tokio runtime. libp2p's event loop and document merges therefore never run on the
HTTP runtime.

- **Configuration:** `P2pConfig::from_lookup` reads three variables:
  - `NOTE_TAKER_P2P_ENABLED` (default `true`);
  - `NOTE_TAKER_P2P_LISTEN` (default `/ip4/0.0.0.0/tcp/0`);
  - `NOTE_TAKER_DEVICE_NAME` (default: the host name, else "Koalino device");
  - `NOTE_TAKER_P2P_PEERS` (comma-separated `…/p2p/<peer id>` addresses for devices that mDNS
    cannot find; an invalid entry stops start-up).
- **Start-up:** `spawn_p2p_task` opens the task's own connections: a document-backed note
  repository and `SqlitePeerStore`. It then loads the sealed node identity, starts listening,
  restores the paired devices, and returns only once all of that worked.
- **Handle:** `P2pHandle` offers `command` (awaits a `P2pCommand` outcome), `notify_local_change`
  (non-blocking, callable from any thread), `snapshot` (status plus listen addresses), and
  `shutdown` (blocking, so callers run it in `spawn_blocking`).
- **Loop:** the task wakes on swarm events, on commands, on scheduled sync retries, on a
  1-second tick that calls `poll_local_changes` and `poll_trust_changes` (so a `cli devices
  revoke` on the same database takes effect without a restart), and on a 10-second tick that redials every
  paired device that is not connected. A transcript saved by the pipeline therefore reaches
  connected devices about a second later, and a device that comes back online within about
  10 seconds.
- **Daemon:** `daemon.rs` starts the task after the pipeline. A disabled or failed task is logged
  and is not fatal. The task is stopped after the Teams bridge at shutdown. Dropping a handle
  without `shutdown` only signals the task to stop.
- **Flutter app:** `ffi.rs` starts the same task when its runtime opens, unless P2P is disabled.
  A task that fails to start is logged, and the rest of the app keeps working.
  - `note_taker_p2p_status` returns the `GET /api/p2p/status` JSON.
  - `note_taker_p2p_command` takes one JSON command and answers `{"ok":...}`. The commands are
    `make_pairable`, `start_pairing`, `confirm_code`, `cancel_pairing`, `accept_request`,
    `reject_request`, `revoke`, and (#121) `set_device_name`; `ffi_p2p.rs` has the table.
- **Device name (#121):** `device_name_settings.rs` saves the owner's name for this device in
  `<db>.device-name` (atomically, through a temporary file). `spawn_p2p_task` lets a saved name
  override `NOTE_TAKER_DEVICE_NAME`, and `set_device_name` (FFI), `PUT /api/p2p/device-name` and
  `cli devices rename` save it and send `P2pCommand::Rename` to a running task. The 10-second
  tick also calls `probe_nearby_names`, so nearby devices open to pairing show their names.
  - A command waits for the task without holding the runtime lock.

## Projects (`project_commands.rs`, Issue #85)

`run_project_command` carries out one JSON Projects command (`list`, `create` (optionally inside
`parent_id`), `move_project` (#142), `rename`,
`delete`, `detail`, `move_meeting`, `meeting_projects`, `add_files`, `add_note`,
`delete_resource`, `read_email`). The app reaches it through `note_taker_project_command`; the
web routes in `server/project_routes.rs` call its functions directly, so both behave alike.
`ProjectFiles` holds the attachment folder and the e-mail reader; the daemon builds it over the
same folder meeting deletions clean. Answers include local `file_path`s only for the app.

## App configuration (`note_taker_configure`)

A desktop process inherits its environment; an Android app has none, and its working directory is
read-only. The app therefore calls `note_taker_configure` once, before anything else, with JSON:

| Field | Required | Becomes |
| --- | --- | --- |
| `data_directory` | yes | The database, `keys/`, `logs/`, `voice_review_clips/`, `attachments/` and `exports/` inside it |
| `device_name` | no | `NOTE_TAKER_DEVICE_NAME` |
| `whisper_model_path` | no | `NOTE_TAKER_WHISPER_MODEL` |
| `keychain_enabled` | no | `NOTE_TAKER_KEYCHAIN_ENABLED` |
| `p2p_enabled` | no | `NOTE_TAKER_P2P_ENABLED` |
| `p2p_peers` | no | `NOTE_TAKER_P2P_PEERS` |
| `export_directory` | no | `NOTE_TAKER_EXPORT_DIR` |

`ffi_settings.rs` validates the JSON: the paths must be absolute, the name must not be blank, and
unknown fields are refused. It stores the values under the variable names the composition roots
already read. The runtime reads every setting through `app_setting`: the configured value first,
then the environment, so desktop development with environment variables still works. The process
environment is never modified.

The call is refused with `2` once the runtime has started, because the runtime would already
have opened its stores elsewhere.

`ffi_developer.rs` (#187) backs Settings → Developer: `note_taker_developer_settings` lists the
few app variables an expert can override (`DEVELOPER_SETTINGS`), validates a new value, and saves
it in `developer-settings.json` in the data folder. `ConfiguredSettings::from_json` loads that file
at start, so the order is: developer override, then the app's configuration, then the environment.
Values apply at the next start, because the pipeline, sync and bridge read their settings once.
Secrets, storage paths, the device ID and test-only variables are never on the list, and a
hand-edited file keeps only allowed, valid values. Values are never logged, only names.

Otherwise the runtime opens lazily, on the first native call. The app calls
`note_taker_start_runtime` right after configuring, on a background isolate, so the database and
P2P sync start with the app instead of when the owner first opens a screen that needs them.

`ffi_logging.rs` (Issue #46) starts the app's file logging in `<data>/logs` when the runtime
first opens, before the stores, so failures while opening them are logged. It holds the logging
runtime in a process-wide cell, because the subscriber can be installed only once while opening
the runtime can be retried. `note_taker_set_log_level` stores the Settings choice in
`<db>.log-level` (the daemon's control file) and applies it; the next start reads it back, and a
missing or damaged file means `info`. `note_taker_export_logs` writes the newest week of logs to
one file with `export_log_files`. `ffi_logging_tests.rs` covers the default, persistence,
refused levels, and the export's paths and errors.

The keyring crate has no Android backend, so on Android Kotlin unwraps the key from the Android
Keystore and passes it to Rust over JNI before Dart starts the runtime. The recording
notification (`RecordingService.kt`, Issue #47) also calls `NativeCore.resumeRecording`,
`pauseRecording`, and `stopRecording`, thin JNI wrappers over the C lifecycle functions, so its
actions work while Dart is in the background. `ffi_android.rs` exports
`Java_io_github_capannone_1dev_koalino_NativeCore_provideEncryptionKey` (Android builds only, with the
`jni` crate). The key never enters the Dart heap. Other hosts can use the C entry point
`note_taker_provide_encryption_key(key, 32)`. Both return 0 when stored and 1 for a null key or
another length, and share `store_host_encryption_key`, which keeps the key in a zeroizing buffer
served through `HostProvidedKeychain`. Without a key, Android start-up fails with status 6
instead of writing a key file. Desktops use the platform keychain unless a key was provided.
`WaitingPassJob.kt` (#87) runs a waiting after-meeting pass while the phone charges with the app
closed, without Dart: `NativeCore.configure`, `startRuntime`, `speechAwaitingPass` and
`runWaitingPass` wrap `note_taker_configure`, `note_taker_start_runtime`,
`note_taker_speech_awaiting_pass` (the meetings whose speech has not taken the pass) and
`note_taker_run_waiting_pass` (one round of the pass, waited for without the runtime lock).

`backup_commands.rs` (#167) holds the backup and restore commands behind Android's Settings and
their status codes (20–27), so host tests run them (`backup_commands_tests.rs`).
`ffi_android.rs` exports `NativeCore.createBackup` and `NativeCore.stageRestore` over them: Kotlin
passes a descriptor detached from the document the owner picked, the passphrase and, for a backup,
the unwrapped key; a restore hands the backup's key back in an array Kotlin wraps with the
Keystore and wipes.

`System.loadLibrary("note_taker_core")` in Kotlin returns the library instance Dart's
`DynamicLibrary.open` already loaded, so both see the same statics (verified on the S23).

Unpaired devices on the LAN can see the node's peer ID through mDNS. They can connect only while
the owner keeps a pairing window open, and even then they cannot sync.

## Teams bot bridge (Issue #33)

`teams_bridge_composition.rs` composes the bridge for the daemon and the desktop app (#123).
The daemon hands it a ready pipeline (`BridgePipeline::ready`). The app's
`ffi_teams_bridge.rs` starts it on a `teams-bridge` thread when the runtime opens on Linux or
Windows, with `BridgePipeline::on_first_meeting`: the pipeline (and its model) is built when the
first browser meeting starts, and a failed build is retried at the next one.
`app_bridge_config` turns the bridge on unless `NOTE_TAKER_TEAMS_BRIDGE_ENABLED=false` and
keeps it off without a secret: it creates the keychain's `teams-bridge-secret` when none exists,
so the browser host and the app agree without an install step. `TeamsBridgeConfig` reads
`NOTE_TAKER_TEAMS_BRIDGE_ENABLED` (default `false`), `_PORT` (`8765`, always on 127.0.0.1),
`_SECRET` (redacted from `Debug`), `_BUFFER_SECONDS` (`10`, range 1–300), and `_SLIDE_DIR`
(default: `slides` next to the database). When the bridge is enabled and the pipeline is
available, `start_teams_bridge_from_environment` does three things:

1. Opens a voice-store connection (Teams identity mappings) and a note-repository connection
   (slide attachments) on a blocking thread.
2. Starts the WebSocket server.
3. Starts the `teams-bridge-pump` thread.

The pump shares the controller's `RecordingManager` and `ChunkAudioStore` and routes each event:

- `MeetingStarted` → `begin_external_meeting`.
- Audio → PCM into the store under `teams-<uuid>`, then `process_external_chunk`, waiting for
  the result. The chunk's audio is always discarded afterwards.
- Slides → `SlideAttachmentStore`.

On shutdown the daemon stops the bridge before the controller: the server closes connections,
the buffer is closed and drained, and the external meeting ends. A disabled, misconfigured, or
failed bridge is logged and the daemon keeps running.

`storage_composition.rs` (`open_encrypted_stores`) is shared by all three roots. It opens the
`encryption_meta` record, unlocks and verifies the embedding key through `KeyManager`
(`NOTE_TAKER_ENCRYPTION_PASSPHRASE`, else the random key in the platform keychain via
`PlatformKeychain`, migrating an old key file from `NOTE_TAKER_ENCRYPTION_KEY_DIR` or
`~/.note_taker`; an unreachable keychain stops start-up with an error;
`NOTE_TAKER_KEYCHAIN_ENABLED=false` uses the key file instead), and only then opens `SqliteNoteRepository` and `SqliteVoiceVectorDB` with one
shared cipher. A wrong key stops start-up with a readable error before any embedding is read.
`open_note_repository` is the one place that opens `SqliteNoteRepository` for production: it
attaches `AutomergeMeetingDocumentCodec`, so every root (daemon, pipeline worker, Teams bridge)
keeps the SQLite index in step with the meeting document.

`daemon.rs` composes the headless Axum server from the encrypted `SqliteNoteRepository` and
`SqliteVoiceVectorDB`, passing them only through `ServerAppState`. HTTP route handlers never use SQLite
directly; they consume repository ports. The daemon defaults to loopback binding and must sit
behind an authenticated reverse proxy before any non-loopback deployment.

`cli.rs` is the interactive terminal composition root. It parses the `core/cli` clap model, opens
the same `NOTE_TAKER_DATABASE_PATH` as the daemon, and passes portable adapters to the thin CLI
service. It fixes structured log filtering at `error` so a user's table/detail output is not mixed
with operational logs.

`ffi.rs` exports `note_taker_get_meetings()` and `note_taker_get_meeting(meeting_id)`, which return
caller-freed JSON with each meeting's ID, display title, and creation time (see `API_SPECS.md`).
`note_taker_rename_meeting(meeting_id, new_title)` (Issue #38) replaces a title through
`application::meeting_rename` and returns a C status: `0`, `1` for null pointers or invalid input,
`2` for storage failures, `3` for an unavailable runtime, and `5` (`FFI_MEETING_NOT_FOUND`).
`note_taker_delete_meeting`, `note_taker_delete_unknown_voice` and `note_taker_delete_saved_person`
delete for good, here and on paired devices (see `API_SPECS.md`). The first two refuse while the
meeting is being recorded. `note_taker_split_meeting` (#88) lists recordings combined into a
meeting on their own again; the meeting JSON names the devices that recorded it. Startup clears review audio left by an earlier run.
`teams_bridge_composition.rs` stores the Teams meeting title on a meeting that has none yet.

`ffi.rs` also exports `note_taker_export_meeting(meeting_id, format, options_json)`, which runs the
shared `ExportManager` and returns caller-freed JSON describing the written file or a structured
error (see `API_SPECS.md`). The CLI binary composes an `ExportWriter` from
`adapters::export::default_exporters()` and `NOTE_TAKER_EXPORT_DIR`.

`ffi.rs` exports the Voice Gallery commands, which call `application::cluster_review`, the same use
case as the CLI and HTTP API. `note_taker_get_unknown_clusters(meeting_id)` returns unresolved
unknown-speaker clusters with optional local review-clip paths. `note_taker_assign_cluster(
cluster_id, identity_name)` assigns a cluster to a named identity and deletes its review audio.
`note_taker_get_voice_library()` lists every meeting's clusters, grouped by voice (#82), and the
saved people, writing no audio. `note_taker_assign_voice_group(cluster_ids_json, identity_name)`
names a group's voices as one person, and `note_taker_prepare_cluster_review_clip(cluster_id)` writes one clip when it is played.
All return caller-freed JSON; see `API_SPECS.md`.

`transcription_settings.rs` keeps Settings' Whisper model, and `ffi_transcription.rs`
exposes them over the C ABI (#67). The same file keeps the transcription glossary: `glossary_source`
reads it fresh for each recording and after-meeting pass (`pipeline_composition.rs` wires both), and
`note_taker_set_glossary(terms)` saves newline-separated terms (0 saved, 1 refused, 2 failed). It
also keeps how many days speech waits to be combined (#93, `keep_speech_days`, 1 to 60, three weeks
by default): `keep_for_source` gives `RefinementWorker::with_keep_for_source` the window fresh at
each round, which also bounds how far back recordings are folded, and
`note_taker_set_keep_speech_days(days)` saves it (0 saved, 1 out of range, 2 failed). It also keeps
whether online-meeting participants' voices are kept in the gallery (#103, `keep_meeting_voices`,
off by default): `teams_bridge_composition.rs` gives `TeamsIdentityMapper::with_voice_keeping` a
closure that reads `keeps_meeting_voices` at every offer. An old saved OpenAI choice is ignored
and dropped at the next save (#117). `device_resources.rs` reads free disk space and total memory.

`llm_settings.rs` keeps the language model settings (#95, #101) in `<db>.llm-settings.json`: the
connections, the default one (the only connection when there is one), the summary template and
the chat prompt (the built-in ones until the owner writes their own; each notes when it changed,
and `FileSharedPrompts` keeps both in step with paired devices, #145), and whether meetings are summarized
automatically, which is on until the owner opts out, and which connection answers paired
devices' summary requests (#107; none until the owner chooses one). Entries edited by hand into something
invalid, or of a removed kind such as `anthropic`, are dropped when read. API keys of online
OpenAI-compatible services (#138) are never in the file: `PlatformLlmKeyring` keeps them in the
platform keychain (`note-taker-llm-<id>`) or, on Android, in `llm-<id>.sealed` under the
Keystore-protected database key; `llm_commands` saves, clears and deletes them and reports only
`has_key`. `forget_sealed_api_keys` deletes the old OpenAI transcription key (`openai_key.sealed`,
#117).

`meeting_summaries.rs` (#95) is shared by the app, the web API and the CLI. `prepare_summary` reads
the labelled transcript, the owner's notes, the image attachments (oldest first, at most 20, each
under 3.75 MB), the project, the language and the meeting's own prompt; `run_summary` asks the
model with no lock held; `store_summary` writes the result into the meeting document in one
transaction. `set_meeting_prompt` checks and stores a meeting's own template.
`summary_export_commands.rs` (#130) exports the stored summary as PDF, text, Word or Markdown,
named `<title> - summary.<ext>`: `export_summary` returns the bytes (the web API's
`GET /api/meetings/{id}/summary/export`), `export_summary_to_file` saves them atomically where the
owner chose (the app's `note_taker_export_summary`).
`load_summary_request`, `store_summary_request` and `store_answered_request` read, write and
answer a paired device's request (#107); the last stores the summary and removes the request in
one write.

`llm_commands.rs` (#95) runs the language model commands shared by the app and the web API, as
`project_commands.rs` does for projects: `settings`, `save_connection`, `delete_connection`,
`update` and `test_connection` (a fixed one-line request with no meeting content). `summarize_with` writes a meeting's summary
with the named or default connection; the caller passes a `prepare` and a `store` step, so its
repository is not held while the model works. `summarize_for_paired_device` does the same with the
asking device's template and labels the summary with this computer's name (#107);
`new_summary_request` and `summary_request_json` build and describe a request.

`auto_summaries.rs` (#101) summarizes finished meetings with the default connection while
`auto_summarize` is on, in rounds on its own thread: at start, every five minutes, and when a
recording stops (in the app). A meeting is due when it was recorded on this device within the
last 7 days, is not folded into another, is not being recorded, its speech is not waiting for
the after-meeting pass, and it has no summary; it is left for a manual summary after 3 failures.
`AutoSummaryHost` is how a process lends it its repositories: `ffi_llm.rs` for the app (started
when the runtime opens) and `server/llm_routes.rs::start_auto_summaries` for the daemon. The CLI
has none: `summary <meeting>` writes one by hand.

The same thread answers paired devices' summary requests (#107) when the owner chose a connection
for them, whatever `auto_summarize` says: at each round it reads every meeting, and between rounds,
every 20 s, only the meetings whose document changed since the last check and the requests it left
for later. It marks a request `working`, writes the summary with the asking device's template and
removes the request, or marks it `failed` with the error code. Another computer's work is taken
over after 30 minutes, and at most 10 requests are sent per hour. `ffi_llm.rs` also gives the app
`note_taker_request_paired_summary`, which writes or cancels a request.

`ffi_speakers.rs` (#186) tells the meeting screen which voice each transcript segment belongs to
(the `voice` field: a saved person, an unknown voice's cluster, or none) and backs
`note_taker_rename_person`, which renames a saved person through `application/person_rename.rs`,
and `note_taker_merge_people` (#188, `application/person_merge.rs`), refused while recording.
Unknown voices are named with `note_taker_assign_cluster`, as in Voices.

`ffi_edits.rs` (#96) gives the app `note_taker_meeting_edit`: the notes, the summary's text and
single transcript segments, through `application/meeting_edits.rs` (the same rules as the web).
It also keeps an answer from the owner's own AI app as the summary (`keep_answer`) and undoes
that (`restore_summary`, #114), and creates a meeting with no recording (`create_meeting`, #129). `meeting_summaries.rs`' `write_handoff`, exposed as
`note_taker_handoff_meeting` in `ffi_llm.rs`, writes the files that hand-off shares; with
`inline` (#126), the message carries the meeting for pasting into a website. Its
`copy_handoff_files` copies the attachments, which always go as files, never as their text.
`project_handoff.rs`' `write_notes_handoff` (#126) does the same for a project's chat, or for
every note (`NotesHandoffScope`): one Markdown file with the meetings (summary, notes,
transcript, oldest first) and copies of the projects' files and each meeting's attachments.
Every note's Markdown always goes as a file only (`document_attached`). `note_taker_handoff_meeting`
takes it with `project_id` or `"all": true`; `project_handoff_tests.rs` specifies it.

`search_commands.rs` (#100) is the search answer shared by the app (`note_taker_search` in
`ffi.rs`), the web (`server/search_routes.rs`) and the CLI (`search`): the request's scope, each
hit as JSON, and stable error codes. `HostSearchSources` gives `application/search.rs` the saved
people's names and project files' text (`ProjectFiles::searchable_text`: text, Markdown and CSV files,
e-mails, PDFs and office documents, through `adapters/documents`, #128).

`notes_chat_commands.rs` (#97) is chat with notes for the app (`ffi_chat.rs`,
`note_taker_chat`), the daemon (`server/chat_routes.rs`) and the CLI (`chat`). `gather_material`
reads what a chat is about. For a meeting: its summary, notes, transcript and photos, then its files' text (#128). For a
project: its files' text and newest images, then its meetings newest first, each followed by its
files' text. For everything: a
list of all meetings and projects, then the meetings (and matching files) that best match the
question. `ask_with` holds the store only while reading and storing, never while the model
answers. A phone asks a paired computer instead (#107): `ask_paired_computer_with` sends the
question and its own chat's turns through `P2pHandle::ask_paired_computer` and keeps the answer
locally, and `answer_for_paired_device` answers on the computer with the connection chosen for
paired devices (`not_offered` without one), reading the computer's own copy of the notes.

`notes_folder.rs` (#98) keeps the opt-in notes folder: a tree of plain files mirroring projects
(`<Project>/`, with their files) and meetings (`<Project or No project>/<YYYY-MM-DD HH-MM Title>/`
with `notes.md`, `summary.md`, a read-only `transcript.md` and `attachments/`). `run_round` reads
edits back (notes and summaries into the meeting document; an edited project file replaces its
resource), imports files dropped into project folders and meetings' `attachments/`, then writes the
tree: deleted or renamed files come back, renamed meetings move their folder, and a file found where
the app writes one is kept as `<name> (on disk)`. `FolderState` (`<db>.notes-folder-state.json`)
remembers what was written, so edits made while the app was closed are recognised.
`notes_folder_commands.rs` is the Settings side shared by the hosts: `NotesFolderKeeper` keeps
`<db>.notes-folder.json`, runs rounds every 30 seconds on a thread of its own (`spawn`), and answers
`get`, `set` and `sync_now`. The app reaches it through `ffi_notes_folder.rs`
(`note_taker_notes_folder`, rounds through `ffi::run_notes_folder_round`), the daemon through
`server/notes_folder_routes.rs`, and the CLI through `notes-folder`. Rounds announce the meetings
they changed to paired devices (not from the CLI, which runs no P2P task). The folder holds clear
copies of the sealed attachment files (#99): it is a decrypted copy by the owner's choice.

`mcp_server.rs` (#112) is the read-only MCP server for the owner's own AI assistant: `McpServer`
answers JSON-RPC lines (`initialize`, `ping`, `tools/list`, `tools/call`) with six reading tools
built on the chat, search and project commands, and `serve` runs it over a reader and a writer.
`AssistantAccessSettings` (`<db>.assistant-access.json`, off by default) is read at every tool
call, so turning access off stops the next read. `access_command` is the Settings side: the switch
and how to connect Claude Code or another MCP client. `mcp.rs` is the `note_taker_mcp` binary the
assistant starts over stdio (logging to the file only, refusing a database path that does not
exist), and `ffi_assistant_access.rs` gives the app `note_taker_assistant_access`.

`call_watch_commands.rs` (#111) is the desktop app's call watch: `CallWatchSettings`
(`<db>.call-watch.json`: `offer`, `auto_stop` and `read_names` (#113), all on by default) and `CallWatcher`, which answers
`poll`, `get` and `set`. A poll reads the microphone's users and reports a call's `started` or
`ended` event once. `ffi_call_watch.rs` gives the app `note_taker_call_watch`; it needs no open
runtime. While `read_names` is on, it turns the Linux desktop's accessibility flag on and says when
call apps must be restarted.

`speaker_name_watcher.rs` (#113) reads who call windows show as speaking beside a recording, on its
own `speaker-name-watcher` thread. `RecordingController::with_speaker_names` starts one per
recording when `read_names` is on; every 2 s while audio is captured it asks the platform's
`CallWindowReader` and adds the names to a `SpeakingTimeline` at the meeting time. At stop the
timeline is attached to the meeting as a sealed `speaker-names-…vtt` (`SpeakerNamesSink`, the
slide store). `pipeline_composition::speaker_names_setup` composes it. The CLI's `call-names`
prints what the reader sees, to check a call app, without opening the notes.

`clear_copies.rs` (#99) gives the app's viewers clear copies of sealed attachments: `ffi.rs`'s
`note_taker_clear_copy` looks an attachment up by ID and decrypts it into the private
`clear-copies` folder beside the attachment folder (`<attachment ID>-<name>`, 0600). A copy is
reused while fresh, removed 30 minutes after it was made once another copy is made, and the folder
is emptied whenever the app starts. The app's start also seals, on a background thread, attachment
files kept in clear from before #99; the daemon does the same, and the CLI seals them in line.

`ffi_language.rs` keeps the apps' default transcription language in `<db>.transcription-language`
(Issue #62). `ffi.rs` applies it when a recording starts, unless one was chosen for that recording.

`p2p_network_routes.rs` owns the P2P task's routes to other networks (Issues #64 and #65): Settings'
relay and peers, applied live and stored in SQLite, and devices found through invites, whose addresses
are stored once they are paired so they can reconnect after a restart.
