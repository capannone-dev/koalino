// Renders the recording lifecycle using Material 3 controls and controller-owned state.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';

/// Displays recording state and delegates user actions to a supplied controller.
final class RecordingScreen extends StatelessWidget {
  /// Creates a screen that observes `recordingController` for its displayable state.
  ///
  /// Inputs: `recordingController` must outlive this widget or be disposed by its owner.
  /// Outputs: A Material 3 recording screen.
  /// Side effects: Registers an animated listener while the widget is mounted.
  const RecordingScreen({required this.recordingController, super.key});

  final RecordingController recordingController;

  /// Builds the status label, error message, and recording-start action.
  ///
  /// Inputs: `context` supplies the Material theme and localization context.
  /// Outputs: A widget tree representing the current controller state.
  /// Side effects: User interaction may request a Rust-owned recording transition through the controller.
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Note Taker')),
      body: Center(
        child: AnimatedBuilder(
          animation: recordingController,
          builder: (BuildContext context, Widget? child) {
            final bool isStarting =
                recordingController.state == RecordingUiState.starting;
            final bool isRecording =
                recordingController.state == RecordingUiState.recording;
            final String statusLabel =
                isRecording ? 'Recording' : 'Ready to record';

            return Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Text(statusLabel,
                    style: Theme.of(context).textTheme.headlineSmall),
                if (recordingController.errorMessage
                    case final String errorMessage)
                  Padding(
                    padding: const EdgeInsets.only(top: 12),
                    child: Text(errorMessage),
                  ),
                const SizedBox(height: 24),
                FilledButton(
                  onPressed: isStarting || isRecording
                      ? null
                      : recordingController.startRecording,
                  child: Text(
                      isStarting ? 'Starting recording' : 'Start recording'),
                ),
              ],
            );
          },
        ),
      ),
    );
  }
}
