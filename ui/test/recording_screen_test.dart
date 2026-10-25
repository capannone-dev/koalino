// Defines widget-first recording-state requirements for the Flutter client.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';

/// Records calls made by a widget through the bridge boundary without loading Rust code.
final class FakeRecordingBridge implements RecordingBridge {
  /// Creates a fake that returns the supplied start outcome.
  FakeRecordingBridge({required this.startResult});

  /// Controls whether the simulated Rust start operation succeeds.
  final bool startResult;

  /// Counts the recording-start requests made through this bridge.
  int startRecordingCallCount = 0;

  @override
  Future<bool> setLogLevel(String filterDirective) async => true;

  @override
  Future<bool> startRecording() async {
    startRecordingCallCount += 1;
    return startResult;
  }
}

/// Verifies that an idle recorder renders the affordance required to begin capture.
void main() {
  testWidgets(
      'start control changes idle state to recording after bridge success',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    final RecordingController recordingController =
        RecordingController(recordingBridge: recordingBridge);

    await tester.pumpWidget(
      MaterialApp(
          home: RecordingScreen(recordingController: recordingController)),
    );

    expect(find.text('Ready to record'), findsOneWidget);
    expect(find.text('Start recording'), findsOneWidget);

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.startRecordingCallCount, 1);
    expect(find.text('Recording'), findsOneWidget);
  });

  testWidgets('failed bridge start keeps the recorder idle and shows an error',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: false);
    final RecordingController recordingController =
        RecordingController(recordingBridge: recordingBridge);

    await tester.pumpWidget(
      MaterialApp(
          home: RecordingScreen(recordingController: recordingController)),
    );

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.startRecordingCallCount, 1);
    expect(find.text('Ready to record'), findsOneWidget);
    expect(find.text('Unable to start recording'), findsOneWidget);
  });
}
