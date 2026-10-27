// Starts the Flutter client with presentation state connected only to the Rust FFI bridge.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';
import 'package:note_taker_ui/controllers/telemetry_dashboard_controller.dart';
import 'package:note_taker_ui/infrastructure/telemetry_bridge.dart';
import 'package:note_taker_ui/presentation/telemetry_dashboard_screen.dart';

/// Starts the Material 3 application using the production Rust recording bridge.
///
/// Inputs: Reads the platform dynamic-library configuration through `RustRecordingBridge`.
/// Outputs: None; Flutter owns the process event loop after this function returns.
/// Side effects: Opens the Rust shared library and renders the application window.
void main() {
  final RecordingController recordingController = RecordingController(
    recordingBridge: RustRecordingBridge(),
  );
  runApp(NoteTakerApp(recordingController: recordingController));
}

/// Configures the application-wide Material 3 theme and root recording screen.
final class NoteTakerApp extends StatelessWidget {
  /// Creates the root app with a controller supplied by the composition root.
  ///
  /// Inputs: `recordingController` is retained by the app for its lifetime.
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
      home: Scaffold(
          body: RecordingScreen(recordingController: recordingController),
          drawer: Drawer(
              child: ListView(children: <Widget>[
            const DrawerHeader(child: Text('Note Taker')),
            ListTile(
                title: const Text('Telemetry Dashboard'),
                onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                    builder: (_) => TelemetryDashboardScreen(
                        controller: TelemetryDashboardController(
                            telemetryBridge: RustTelemetryBridge())))))
          ]))),
    );
  }
}
