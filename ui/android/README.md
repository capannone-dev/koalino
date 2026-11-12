<!-- This directory is Flutter-generated Android runner scaffolding for the Flutter client. -->

# Android runner and background recording integration

Flutter owns this standard Gradle and Android embedding scaffold. Application logic remains in
`ui/lib`; Rust is invoked only through the Dart bridge there. This guide documents the platform
wiring required for background recording and lock-screen controls without placing Android service
code in the Rust core.

## Native libraries and device sync

`tools/cross/build-flutter-native.sh android` builds the Rust core for `arm64-v8a` and copies it
into `app/src/main/jniLibs/arm64-v8a/` with the NDK's `libc++_shared.so`, which whisper.cpp links
against; Gradle packages both. The directory is gitignored.

The manifest requests `INTERNET`, `ACCESS_NETWORK_STATE`, `ACCESS_WIFI_STATE`, and
`CHANGE_WIFI_MULTICAST_STATE` for libp2p and mDNS. `MainActivity` holds a Wi-Fi multicast lock
while the app is visible, because Android drops mDNS packets otherwise; background sync is
Issue #47.

The root `build.gradle.kts` raises plugin libraries' `compileSdk` to 36 after they are evaluated,
because file_picker 8 still compiles against 34 while its AndroidX dependencies require 36.

## Database key (Android Keystore)

`EncryptionKeyVault.kt` keeps the 32-byte database key in `keys/encryption.key.wrapped`
(`[version 1][12-byte IV][ciphertext + tag]`), wrapped with the non-exportable AES-256-GCM
Keystore key `note_taker_database_key_wrapper`, in StrongBox when the phone has one and in the TEE
otherwise. On first use it creates a random key, or wraps the plain `encryption.key` left by
earlier builds, checks the wrapped file unwraps to the same key, and only then deletes the plain
file. When Dart asks on the `note_taker/key_vault` channel, `MainActivity` unwraps it on a
background thread and passes it straight to Rust with `NativeCore.provideEncryptionKey` (JNI,
`core/bin/ffi_android.rs`), then zero-fills its copy; the key never reaches Dart. A Keystore
failure answers `keystore_unavailable` and a refused key or missing JNI method `key_rejected`;
the app shows both as the keychain error with Retry.

## Whisper model

The APK ships `ggml-base.bin` as an uncompressed asset (`assets/models/`, staged by
`tools/cross/build-flutter-native.sh android` into `ui/native/android-assets`, added as an asset
source in `app/build.gradle.kts`). whisper.cpp needs a file path, so `WhisperModel.kt` copies it
into the app's data directory through a temporary file on first start. The APK is about 218 MB;
Play Store distribution would need an asset pack instead.

## Background recording (Issue #47)

`RecordingService.kt` is a foreground service of type `microphone` (permissions
`FOREGROUND_SERVICE`, `FOREGROUND_SERVICE_MICROPHONE`), so capture continues with the app in the
background or the screen off. Dart starts, updates, and ends it on `note_taker/recording_service`
(`show(state, title)`); its ongoing notification offers Pause/Resume and Stop, which call the core
through `NativeCore` on a background thread and report `stateChanged` back to Dart. The
lock-screen (public) version omits the meeting title. A tap opens the app. Verified on the S23:
recording from the background while another app played audio, pause and resume from the
notification, and a stop that finished transcription and ended the service.

## Charging and the after-meeting pass (#87)

A phone runs the after-meeting transcript pass only while charging, and the core cannot read the
battery. `ChargingReporter.kt` registers for the sticky `ACTION_BATTERY_CHANGED` broadcast while
`MainActivity` exists and passes the state to `NativeCore.setCharging` on a background thread: on
every change, and on every battery report while charging. The core remembers a report made before
it has opened the database. Charging with speech left from an earlier run starts the pass without
a new recording; that loads the Whisper model, which is why the call never runs on the main
thread. The pass runs while the app's process runs: Android may freeze a cached app soon after it
leaves the screen, so a pass pauses then and continues when the app is opened again.

## Microphone

The Rust core records the microphone itself (`cpal` on AAudio, Issue #45), so the app declares
`RECORD_AUDIO`; on Android 13 and later the same request asks for `POST_NOTIFICATIONS` (once if
refused). `MicrophonePermission.kt` serves `note_taker/microphone`: `ensureGranted` answers
true at once when granted, otherwise shows the system dialog (`MainActivity` forwards
`onRequestPermissionsResult`); a permanent refusal answers false without a dialog. Framework APIs
only, no AndroidX.

## App lock

`AppLock.kt` serves `note_taker/app_lock`: the setting (SharedPreferences
`note_taker_app_lock`), whether a screen lock is set, and the framework `BiometricPrompt`, which
allows strong biometrics or the screen lock's PIN, pattern, or password (Android 10 and later; no
AndroidX library). While the lock is on, the recent-apps screen shows no preview of the app
(Android 13 and later); screenshots stay allowed. When to lock is decided in Dart
(`AppLockController`). The lock covers only the UI: the Rust core, recording, and sync keep
running, and future lock-screen recording controls (#47) work without unlocking the app. Verified on the Galaxy S23 (StrongBox; the migrated database opened).

For a development build that must reach devices mDNS cannot see, such as VMs through
`adb reverse`, compile peer addresses in:
`flutter build apk --dart-define=NOTE_TAKER_P2P_PEERS=/ip4/127.0.0.1/tcp/14001/p2p/<peer id>`.

## Lifecycle contract

The Rust library owns one process-wide recorder behind a `OnceLock<Mutex<...>>`. Any Android UI,
notification receiver, or foreground service thread may call these no-argument C ABI functions:

- `note_taker_start_recording()` — starts Idle or resumes Paused/Interrupted.
- `note_taker_pause_recording()` — pauses Recording while retaining the session.
- `note_taker_stop_recording()` — terminally stops Recording, Paused, or Interrupted.
- `note_taker_handle_audio_interruption()` — reports an OS audio-focus or input-stream suspension.
- `note_taker_get_current_state()` — reads a lock-screen-safe state code: Idle `10`, Recording
  `11`, Paused `12`, Interrupted `13`, Stopped `14`, Error `15`.

All mutation calls return `0` on success. Values `1`, `2`, and `3` retain the core’s invalid
argument, rejected-operation, and unavailable-runtime meanings. Callers must treat a nonzero
lifecycle result as a no-op and refresh notification state through `note_taker_get_current_state()`.

## Foreground-service wiring steps

1. Declare microphone and foreground-service microphone permissions in the Android manifest, then
   request microphone permission before issuing the first Start command.
2. Register a foreground recording service using the platform’s microphone foreground-service
   type. Start it before, or atomically with, `note_taker_start_recording()` so Android does not
   reclaim the process when the Flutter activity backgrounds.
3. Load the same Rust shared library in the service process/thread that Flutter uses. Do not create
   a second native runtime or retain an unmanaged pointer to Rust state.
4. Create a persistent notification with Start, Pause, and Stop pending-intent actions. Each action
   must call exactly one corresponding FFI lifecycle function, then query current state and redraw
   the notification.
5. Reflect Recording/Paused/Interrupted state in the notification’s media-style controls. Hide or
   disable Start while Recording; expose Start as Resume while Paused or Interrupted; expose Stop
   for all nonterminal active states.
6. Register for audio-focus loss, microphone privacy toggles, and service task removal. On a
   transient audio suspension, call `note_taker_handle_audio_interruption()` rather than Stop. On
   focus regain, offer Resume and call Start only after user/service policy permits it.
7. Keep all raw audio handling in the platform capture adapter. Never put samples, user names,
   tokens, or Rust pointers in notification extras, logs, or intents.
8. On explicit Stop, invoke `note_taker_stop_recording()`, cancel the foreground notification, and
   stop the service. Do not stop the service after a recoverable interruption.

## Lock-screen and process-death behavior

Use the persistent foreground notification as the lock-screen/notification-shade control surface.
Every action must query state after its command, because a concurrent Flutter action can win the
mutex first. If Android kills the process, do not infer a persisted Recording state: start a fresh
native runtime and reconcile with durable application metadata before exposing controls. The Rust
state machine intentionally does not retain raw audio or background-service tokens.

## Sharing logs (Issue #46)

`SharedFiles.kt` answers `shareFile(path, mimeType)` on `note_taker/share`: it opens the system
share sheet (`ACTION_SEND` with a read grant) for one file in the app's `cache/shared` folder,
where Settings' Share logs writes the export. `SharedFileProvider` serves that folder read-only
as `content://<applicationId>.shared/<name>`. It is not exported, so only the app picked in the
sheet can read the shared file, and a path outside the folder (including through `..` or a
link) is refused. It is our own small provider instead of androidx's `FileProvider`, so no
library was added. Verified on the S23: the sheet showed the file's name (read through the
grant), and `adb shell content query` was refused.

For handing a meeting to the owner's own AI app (#114), `SharedFiles.kt` has two more calls:

- `shareFiles(paths, text, packageName?)` sends the message and the files of
  `cache/shared/handoff` together (`ACTION_SEND`, or `ACTION_SEND_MULTIPLE` for several files,
  with read grants). It goes to the Claude or ChatGPT app when one is named and installed, else
  through the share sheet.
- `installedApps` lists which of those two apps are installed. The manifest's `<queries>` makes
  them visible.

The provider also serves `handoff/<name>` and types files by extension, Markdown as plain text.
Both apps accept `SEND` and `SEND_MULTIPLE` of any type (checked on the S23 with
`cmd package query-activities`).

## Sharing to a project (Issue #85)

The activity accepts `SEND` and `SEND_MULTIPLE` for e-mails (`message/rfc822`), PDFs, pictures
and text (Markdown included), so mail apps, file managers and galleries offer Koalino in their
share sheet. `IncomingShares.kt` copies each share at once, on a background thread, into its own
folder under `cache/incoming` (a shared URI is readable only while the share lasts):

- Only `content://` URIs are read. A `file://` URI from another app could name this app's own
  private files, such as the database or keys, which must never end up in a project.
- Names come from the sender's display name, reduced to one safe path component; an e-mail
  without a name gets `.eml`, so Rust recognises it. A text-only share becomes a `.txt` file named
  after its subject.
- A file over 100 MiB (Rust's attachment limit) or one that can't be read is dropped and counted.
- A share that starts the app is taken in `onCreate` (not again when a killed activity is
  restored); one reaching the running app arrives through `onNewIntent` (`singleTop`).

Dart takes the share on `note_taker/incoming_shares` (`takePending`, and `sharesArrived` while it
runs), asks which project it goes into, and calls `release(directory)` once Rust has copied the
files. Folders left behind are removed after a day.
