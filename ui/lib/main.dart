// Starts the Flutter client with presentation state connected only to the Rust FFI bridges.

import 'dart:async';
import 'dart:io';
import 'dart:ui' show AppExitType;

import 'package:audioplayers/audioplayers.dart';
// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';
import 'package:note_taker_ui/controllers/meeting_edit_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/search_bridge.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/controllers/app_lock_controller.dart';
import 'package:note_taker_ui/controllers/core_startup_controller.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/controllers/live_transcript_controller.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/controllers/network_settings_controller.dart';
import 'package:note_taker_ui/controllers/photo_capture_controller.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:note_taker_ui/controllers/transcription_route_controller.dart';
import 'package:note_taker_ui/controllers/telemetry_dashboard_controller.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/app_configuration.dart';
import 'package:note_taker_ui/infrastructure/app_lock_bridge.dart';
import 'package:note_taker_ui/infrastructure/host_key.dart';
import 'package:note_taker_ui/infrastructure/incoming_share_bridge.dart';
import 'package:note_taker_ui/infrastructure/language_bridge.dart';
import 'package:note_taker_ui/infrastructure/log_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_attachment_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/infrastructure/photo_camera.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/controllers/meeting_export_controller.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';
import 'package:note_taker_ui/infrastructure/recording_notifier.dart';
import 'package:note_taker_ui/infrastructure/telemetry_bridge.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/presentation/app_lock_gate.dart';
import 'package:note_taker_ui/presentation/app_theme.dart';
import 'package:note_taker_ui/presentation/core_startup_gate.dart';
import 'package:note_taker_ui/presentation/devices_screen.dart';
import 'package:note_taker_ui/presentation/home_shell.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';
import 'package:note_taker_ui/presentation/photo_capture_screen.dart';
import 'package:note_taker_ui/presentation/projects_screen.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';
import 'package:note_taker_ui/presentation/share_to_project.dart';
import 'package:note_taker_ui/presentation/telemetry_dashboard_screen.dart';
import 'package:note_taker_ui/presentation/voice_gallery_screen.dart';

/// Configures the Rust core, then starts the Material 3 application over the Rust bridges.
///
/// Inputs: The platform's application-support directory, from `configureCore`.
/// Outputs: None; Flutter owns the process event loop after this function returns.
/// Side effects: Creates the app's data directory, opens the Rust shared library, stores its
/// settings before any other native call, renders the application window, then opens the
/// runtime (database, encryption key, and device sync) on a background isolate. The screens stay
/// hidden behind `CoreStartupGate` until it is open.
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // A non-zero status leaves the core on its defaults; the screens then report their own errors.
  await configureCore();
  final RustRecordingBridge recordingBridge = RustRecordingBridge();
  final RecordingController recordingController = RecordingController(
    recordingBridge: recordingBridge,
    microphonePermission: platformMicrophonePermission(),
    recordingNotifier: platformRecordingNotifier(),
  );
  // Lives as long as the app, so a choice survives switching destinations.
  final TranscriptionLanguageController languageController =
      TranscriptionLanguageController(
    languageBridge: const RustLanguageBridge(),
    recordingController: recordingController,
  );
  unawaited(languageController.load());
  // Lives as long as the recording, so the transcript keeps growing on any destination.
  final LiveTranscriptController liveTranscriptController =
      LiveTranscriptController(
    recordingController: recordingController,
    recordingBridge: recordingBridge,
    meetingBridge: const RustMeetingBridge(),
  );
  final CoreStartupController coreStartupController = CoreStartupController(
    // Android keeps the key in the Keystore and Kotlin hands it to Rust over JNI first;
    // desktops reach their keychain from Rust.
    startCore: () => startCoreWithHostKey(
      provideHostKey: Platform.isAndroid
          ? () async => const AndroidKeyVault()
              .provideEncryptionKey(await appKeyDirectory())
          : null,
      startCore: startCore,
    ),
  );
  // Android only: the lock covers the UI; the core starts underneath it either way.
  final AppLockController? appLockController = Platform.isAndroid
      ? AppLockController(appLockBridge: const AndroidAppLockBridge())
      : null;
  runApp(NoteTakerApp(
    recordingController: recordingController,
    liveTranscriptController: liveTranscriptController,
    activeMeetingId: recordingBridge.activeMeetingId,
    coreStartupController: coreStartupController,
    appLockController: appLockController,
    languageController: languageController,
    incomingShareBridge: platformIncomingShareBridge(),
  ));
  unawaited(appLockController?.initialize());
  // Starts after the first frame, so the waiting message shows during a keychain prompt.
  unawaited(coreStartupController.start());
}

/// Configures the application-wide Material 3 light and dark themes and the adaptive navigation shell.
final class NoteTakerApp extends StatelessWidget {
  /// Creates the root app with a controller supplied by the composition root.
  ///
  /// Inputs: `recordingController` is retained by the app for its lifetime, so an active
  /// recording survives switching destinations. `coreStartupController`, when given, holds the
  /// screens back until the Rust core is open (tests that fake every bridge omit it).
  /// `liveTranscriptController`, when given, shows the transcript growing on the Record screen;
  /// `activeMeetingId` adds Take photos there on platforms with a camera.
  /// `appLockController`, when given (Android), covers every screen while the app is locked
  /// and adds the App lock switch to Settings. `languageController`, when given, adds the language
  /// selector to the Record screen and the default language to Settings.
  /// `incomingShareBridge`, when given (Android), offers files shared from other apps to a
  /// project once the core is open (Issue #85).
  /// Outputs: A configured Material application.
  /// Side effects: None.
  const NoteTakerApp({
    required this.recordingController,
    this.liveTranscriptController,
    this.activeMeetingId,
    this.coreStartupController,
    this.appLockController,
    this.languageController,
    this.incomingShareBridge,
    super.key,
  });

  final RecordingController recordingController;
  final TranscriptionLanguageController? languageController;
  final LiveTranscriptController? liveTranscriptController;

  /// Reads the meeting being recorded, for Take photos on the Record screen (Issue #54).
  final Future<String?> Function()? activeMeetingId;
  final CoreStartupController? coreStartupController;
  final AppLockController? appLockController;
  final IncomingShareBridge? incomingShareBridge;

  /// Builds the root Material 3 application.
  ///
  /// Inputs: `context` supplies inherited Flutter configuration.
  /// Outputs: The root `MaterialApp` widget.
  /// Side effects: None.
  @override
  Widget build(BuildContext context) {
    final Widget homeShell = HomeShell(
      recordingScreenBuilder: (BuildContext context) => RecordingScreen(
        recordingController: recordingController,
        languageController: languageController,
        liveTranscriptController: liveTranscriptController,
        photoScreenBuilder: _photoScreenBuilder,
        attachmentsController: _recordingAttachmentsController,
        activeMeetingId: activeMeetingId,
      ),
      meetingsScreenBuilder: (BuildContext context) =>
          const _MeetingsDestination(),
      voicesScreenBuilder: (BuildContext context) =>
          const _VoiceGalleryDestination(),
      projectsScreenBuilder: (BuildContext context) => ProjectsScreen(
        controller: _projectsController,
        meetingScreenBuilder: (BuildContext context, String meetingId) =>
            _MeetingsDestination(openMeetingId: meetingId),
        summariesController: _summariesController,
        searchBridge: const SearchBridge(),
        meetingMomentBuilder:
            (BuildContext context, String meetingId, int? startTimeMs) =>
                _MeetingsDestination(
                    openMeetingId: meetingId, openAtMs: startTimeMs),
      ),
      settingsScreenBuilder: (BuildContext context) => _SettingsDestination(
        appLockController: appLockController,
        languageController: languageController,
      ),
    );
    final IncomingShareBridge? shareBridge = incomingShareBridge;
    // Inside the startup gate, so a share waits until the core can store it.
    final Widget shell = shareBridge == null
        ? homeShell
        : IncomingShareListener(
            shareBridge: shareBridge,
            projectsController: _projectsController,
            child: homeShell,
          );
    final CoreStartupController? startup = coreStartupController;
    final AppLockController? appLock = appLockController;
    return MaterialApp(
      title: 'Note Taker',
      theme: noteTakerTheme(Brightness.light),
      darkTheme: noteTakerTheme(Brightness.dark),
      // Follows the system's light or dark setting, including changes while the app runs.
      themeMode: ThemeMode.system,
      // Wraps the navigator, so pushed screens and dialogs are covered as well.
      builder: appLock == null
          ? null
          : (BuildContext context, Widget? navigator) =>
              AppLockGate(controller: appLock, child: navigator!),
      home: startup == null
          ? shell
          : CoreStartupGate(controller: startup, onQuit: _quit, child: shell),
    );
  }
}

/// Closes the app: Android finishes the activity; desktops exit the process.
void _quit() {
  if (Platform.isAndroid) {
    unawaited(SystemNavigator.pop());
  } else {
    unawaited(ServicesBinding.instance.exitApplication(AppExitType.required));
  }
}

/// Owns a meeting controller for as long as the Meetings destination is shown, or one meeting's
/// page opened from a project (`openMeetingId`).
final class _MeetingsDestination extends StatefulWidget {
  const _MeetingsDestination({this.openMeetingId, this.openAtMs});

  /// The meeting to show alone, or `null` for the meeting list.
  final String? openMeetingId;

  /// The transcript passage to scroll `openMeetingId` to, such as a search result (#100).
  final int? openAtMs;

  @override
  State<_MeetingsDestination> createState() => _MeetingsDestinationState();
}

/// Creates and disposes the destination's controller.
final class _MeetingsDestinationState extends State<_MeetingsDestination> {
  final MeetingListController _controller =
      MeetingListController(meetingBridge: const RustMeetingBridge());
  final MeetingExportController _exportController = MeetingExportController(
    exportBridge: const RustExportBridge(),
    exportSaver: const FilePickerExportSaver(),
  );
  final MeetingAttachmentsController _attachmentsController =
      MeetingAttachmentsController(
    attachmentBridge: const RustMeetingAttachmentBridge(),
    filesPicker: const PlatformAttachmentFilesPicker(),
  );

  @override
  void dispose() {
    _controller.dispose();
    _exportController.dispose();
    _attachmentsController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (widget.openMeetingId case final String meetingId) {
      return MeetingDetailScreen(
        controller: _controller,
        meetingId: meetingId,
        exportController: _exportController,
        attachmentsController: _attachmentsController,
        photoScreenBuilder: _photoScreenBuilder,
        speakersScreenBuilder: (BuildContext context, String meetingId) =>
            _VoiceGalleryDestination(meetingId: meetingId),
        projectsController: _projectsController,
        summariesController: _summariesController,
        editController: _meetingEditController,
        focusStartMs: widget.openAtMs,
      );
    }
    return MeetingListScreen(
      controller: _controller,
      exportController: _exportController,
      attachmentsController: _attachmentsController,
      photoScreenBuilder: _photoScreenBuilder,
      speakersScreenBuilder: (BuildContext context, String meetingId) =>
          _VoiceGalleryDestination(meetingId: meetingId),
      projectsController: _projectsController,
      summariesController: _summariesController,
      editController: _meetingEditController,
      searchBridge: const SearchBridge(),
    );
  }
}

/// AI connections and meeting summaries (#95), shared by Settings and every meeting list; a
/// summary keeps being written when its page is left, so this lives as long as the app.
final SummariesController _summariesController =
    SummariesController(bridge: const LlmBridge());

/// The owner's meeting notes and transcript corrections (#96), shared by every meeting view.
final MeetingEditController _meetingEditController = MeetingEditController();

/// Projects state (Issue #85), shared by the Projects tab, meeting pages and files shared from
/// other apps; it holds no resources, so it lives as long as the app.
final ProjectsController _projectsController = ProjectsController(
  projectBridge: const ProjectBridge(),
  meetingBridge: const RustMeetingBridge(),
  filesPicker: const PlatformProjectFilesPicker(),
);

/// Attaches files to the meeting being recorded from the Record screen; it holds no resources,
/// so it lives as long as the app.
final MeetingAttachmentsController _recordingAttachmentsController =
    MeetingAttachmentsController(
  attachmentBridge: const RustMeetingAttachmentBridge(),
  filesPicker: const PlatformAttachmentFilesPicker(),
);

/// Builds the camera for a meeting, or `null` on platforms without camera support (Linux),
/// which then offer file attachments only (Issue #54).
final MeetingScreenBuilder? _photoScreenBuilder = isCameraSupported
    ? (BuildContext context, String meetingId) =>
        _PhotoCaptureDestination(meetingId: meetingId)
    : null;

/// Owns the camera and its controller while the photo screen of one meeting is shown, so
/// leaving the screen turns the camera off.
final class _PhotoCaptureDestination extends StatefulWidget {
  const _PhotoCaptureDestination({required this.meetingId});

  final String meetingId;

  @override
  State<_PhotoCaptureDestination> createState() =>
      _PhotoCaptureDestinationState();
}

/// Creates and disposes the screen's controller.
final class _PhotoCaptureDestinationState
    extends State<_PhotoCaptureDestination> {
  late final PhotoCaptureController _controller = PhotoCaptureController(
    camera: PluginPhotoCamera(),
    attachmentBridge: const RustMeetingAttachmentBridge(),
    meetingId: widget.meetingId,
  );

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      PhotoCaptureScreen(controller: _controller);
}

/// Owns a voice gallery controller while the gallery is shown: for one meeting when `meetingId`
/// is given, otherwise for all of them.
final class _VoiceGalleryDestination extends StatefulWidget {
  const _VoiceGalleryDestination({this.meetingId});

  final String? meetingId;

  @override
  State<_VoiceGalleryDestination> createState() =>
      _VoiceGalleryDestinationState();
}

/// Creates and disposes the screen's controller and its review-clip player, so leaving the
/// screen stops playback and releases the platform player.
final class _VoiceGalleryDestinationState
    extends State<_VoiceGalleryDestination> {
  final AudioPlayer _reviewClipPlayer = AudioPlayer();
  late final VoiceGalleryController _controller = VoiceGalleryController(
    voiceGalleryBridge: RustVoiceGalleryBridge(audioPlayer: _reviewClipPlayer),
    meetingId: widget.meetingId,
  );

  @override
  void dispose() {
    _controller.dispose();
    unawaited(_reviewClipPlayer.dispose());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      VoiceGalleryScreen(voiceGalleryController: _controller);
}

/// Owns a telemetry controller, so its polling timer stops when the Settings page is closed.
final class _TelemetryDestination extends StatefulWidget {
  const _TelemetryDestination();

  @override
  State<_TelemetryDestination> createState() => _TelemetryDestinationState();
}

/// Creates and disposes the destination's controller.
final class _TelemetryDestinationState extends State<_TelemetryDestination> {
  final TelemetryDashboardController _controller =
      TelemetryDashboardController(telemetryBridge: RustTelemetryBridge());

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) =>
      TelemetryDashboardScreen(controller: _controller);
}

/// Owns the log settings controller while Settings is shown.
final class _SettingsDestination extends StatefulWidget {
  const _SettingsDestination({
    required this.appLockController,
    required this.languageController,
  });

  /// Android's app lock, or `null` on platforms without it.
  final AppLockController? appLockController;

  /// The transcription language, owned by the app.
  final TranscriptionLanguageController? languageController;

  @override
  State<_SettingsDestination> createState() => _SettingsDestinationState();
}

/// Creates the controller, reads the stored level, and disposes it when Settings is left.
final class _SettingsDestinationState extends State<_SettingsDestination> {
  final LogSettingsController _logSettingsController = LogSettingsController(
    logBridge: const RustLogBridge(),
    exportSaver: const FilePickerExportSaver(dialogTitle: 'Export logs'),
    // Android shares from `cache/shared`, the folder `SharedFiles.kt` serves.
    logSharer: Platform.isAndroid ? const AndroidLogSharer() : null,
    sharedDirectory: () async =>
        Directory('${(await getTemporaryDirectory()).path}/shared'),
  );

  final TranscriptionRouteController _transcriptionRouteController =
      TranscriptionRouteController(p2pBridge: const RustP2pBridge());

  final NetworkSettingsController _networkSettingsController =
      NetworkSettingsController(p2pBridge: const RustP2pBridge());

  @override
  void initState() {
    super.initState();
    unawaited(_logSettingsController.load());
    unawaited(_transcriptionRouteController.load());
    unawaited(_networkSettingsController.load());
  }

  @override
  void dispose() {
    _logSettingsController.dispose();
    _transcriptionRouteController.dispose();
    _networkSettingsController.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => SettingsScreen(
        logSettingsController: _logSettingsController,
        appLockController: widget.appLockController,
        telemetryScreenBuilder: (BuildContext context) =>
            const _TelemetryDestination(),
        devicesScreenBuilder: (BuildContext context) =>
            const _DevicesDestination(),
        transcriptionRouteController: _transcriptionRouteController,
        languageController: widget.languageController,
        networkSettingsController: _networkSettingsController,
        transcriptionSettingsBridge: const RustTranscriptionSettingsBridge(),
        summariesController: _summariesController,
      );
}

/// Owns a devices controller and polls only while the Devices destination is shown.
final class _DevicesDestination extends StatefulWidget {
  const _DevicesDestination();

  @override
  State<_DevicesDestination> createState() => _DevicesDestinationState();
}

/// Creates the controller, starts polling, and disposes both when the destination is left.
final class _DevicesDestinationState extends State<_DevicesDestination> {
  final DevicesController _controller =
      DevicesController(p2pBridge: const RustP2pBridge());

  @override
  void initState() {
    super.initState();
    _controller.startPolling();
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => DevicesScreen(controller: _controller);
}
