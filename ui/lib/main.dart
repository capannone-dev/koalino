// Starts the Flutter client with presentation state connected only to the Rust FFI bridges.

import 'dart:async';
import 'dart:io';
import 'dart:ui' show AppExitType;

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:note_taker_ui/controllers/core_startup_controller.dart';
import 'package:note_taker_ui/controllers/attachment_controller.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/controllers/telemetry_dashboard_controller.dart';
import 'package:note_taker_ui/infrastructure/app_configuration.dart';
import 'package:note_taker_ui/infrastructure/attachment_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';
import 'package:note_taker_ui/infrastructure/telemetry_bridge.dart';
import 'package:note_taker_ui/presentation/app_theme.dart';
import 'package:note_taker_ui/presentation/attachment_import_screen.dart';
import 'package:note_taker_ui/presentation/core_startup_gate.dart';
import 'package:note_taker_ui/presentation/devices_screen.dart';
import 'package:note_taker_ui/presentation/home_shell.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';
import 'package:note_taker_ui/presentation/telemetry_dashboard_screen.dart';

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
  final RecordingController recordingController = RecordingController(
    recordingBridge: RustRecordingBridge(),
  );
  final CoreStartupController coreStartupController =
      CoreStartupController(startCore: startCore);
  runApp(NoteTakerApp(
    recordingController: recordingController,
    coreStartupController: coreStartupController,
  ));
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
  /// Outputs: A configured Material application.
  /// Side effects: None.
  const NoteTakerApp({
    required this.recordingController,
    this.coreStartupController,
    super.key,
  });

  final RecordingController recordingController;
  final CoreStartupController? coreStartupController;

  /// Builds the root Material 3 application.
  ///
  /// Inputs: `context` supplies inherited Flutter configuration.
  /// Outputs: The root `MaterialApp` widget.
  /// Side effects: None.
  @override
  Widget build(BuildContext context) {
    final Widget shell = HomeShell(
      recordingScreenBuilder: (BuildContext context) =>
          RecordingScreen(recordingController: recordingController),
      meetingsScreenBuilder: (BuildContext context) =>
          const _MeetingsDestination(),
      telemetryScreenBuilder: (BuildContext context) =>
          const _TelemetryDestination(),
      devicesScreenBuilder: (BuildContext context) =>
          const _DevicesDestination(),
    );
    final CoreStartupController? startup = coreStartupController;
    return MaterialApp(
      title: 'Note Taker',
      theme: noteTakerTheme(Brightness.light),
      darkTheme: noteTakerTheme(Brightness.dark),
      // Follows the system's light or dark setting, including changes while the app runs.
      themeMode: ThemeMode.system,
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

/// Owns a meeting controller for as long as the Meetings destination is shown.
final class _MeetingsDestination extends StatefulWidget {
  const _MeetingsDestination();

  @override
  State<_MeetingsDestination> createState() => _MeetingsDestinationState();
}

/// Creates and disposes the destination's controller.
final class _MeetingsDestinationState extends State<_MeetingsDestination> {
  final MeetingListController _controller =
      MeetingListController(meetingBridge: const RustMeetingBridge());

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return MeetingListScreen(
      controller: _controller,
      attachmentScreenBuilder: (BuildContext context) => AttachmentImportScreen(
        attachmentController: AttachmentController(
          attachmentBridge: RustAttachmentBridge(),
          attachmentFilePicker: PlatformAttachmentFilePicker(),
        ),
      ),
    );
  }
}

/// Owns a telemetry controller, so its polling timer stops when the destination is left.
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
