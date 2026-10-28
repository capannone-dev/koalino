// Starts the Flutter client with presentation state connected only to the Rust FFI bridges.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
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
import 'package:note_taker_ui/presentation/attachment_import_screen.dart';
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
/// settings before any other native call, and renders the application window.
Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();
  // A non-zero status leaves the core on its defaults; the screens then report their own errors.
  await configureCore();
  final RecordingController recordingController = RecordingController(
    recordingBridge: RustRecordingBridge(),
  );
  runApp(NoteTakerApp(recordingController: recordingController));
}

/// Configures the application-wide Material 3 theme and the adaptive navigation shell.
final class NoteTakerApp extends StatelessWidget {
  /// Creates the root app with a controller supplied by the composition root.
  ///
  /// Inputs: `recordingController` is retained by the app for its lifetime, so an active
  /// recording survives switching destinations.
  /// Outputs: A configured Material application.
  /// Side effects: None.
  const NoteTakerApp({required this.recordingController, super.key});

  final RecordingController recordingController;

  /// Builds the root Material 3 application.
  ///
  /// Inputs: `context` supplies inherited Flutter configuration.
  /// Outputs: The root `MaterialApp` widget.
  /// Side effects: None.
  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Note Taker',
      theme: ThemeData(colorSchemeSeed: Colors.indigo, useMaterial3: true),
      home: HomeShell(
        recordingScreenBuilder: (BuildContext context) =>
            RecordingScreen(recordingController: recordingController),
        meetingsScreenBuilder: (BuildContext context) =>
            const _MeetingsDestination(),
        telemetryScreenBuilder: (BuildContext context) =>
            const _TelemetryDestination(),
        devicesScreenBuilder: (BuildContext context) =>
            const _DevicesDestination(),
      ),
    );
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
