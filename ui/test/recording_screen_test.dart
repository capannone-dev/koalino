// Defines widget-first recording-state and meeting-title requirements for the Flutter client.

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

  /// Titles passed to `startRecordingWithTitle`, in call order (`null` for untitled starts).
  final List<String?> requestedTitles = <String?>[];

  /// Counts the recording-start requests made through this bridge.
  int get startRecordingCallCount => requestedTitles.length;

  @override
  Future<bool> setLogLevel(String filterDirective) async => true;

  @override
  Future<bool> startRecording() => startRecordingWithTitle(null);

  @override
  Future<bool> startRecordingWithTitle(String? title) async {
    requestedTitles.add(title);
    return startResult;
  }
}

/// Pumps the recording screen around a controller backed by `recordingBridge`.
Future<void> pumpRecordingScreen(
    WidgetTester tester, FakeRecordingBridge recordingBridge) async {
  await tester.pumpWidget(
    MaterialApp(
      home: RecordingScreen(
        recordingController:
            RecordingController(recordingBridge: recordingBridge),
      ),
    ),
  );
}

/// Verifies the recording lifecycle and the optional meeting title sent to Rust.
void main() {
  testWidgets(
      'start control changes idle state to recording after bridge success',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

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
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.startRecordingCallCount, 1);
    expect(find.text('Ready to record'), findsOneWidget);
    expect(find.text('Unable to start recording'), findsOneWidget);
  });

  testWidgets('an optional meeting title field exists and accepts text',
      (WidgetTester tester) async {
    await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true));

    final Finder titleField = find.byType(TextField);
    expect(titleField, findsOneWidget);
    expect(find.text('Meeting title'), findsOneWidget);
    expect(find.text('Enter meeting title (optional)'), findsOneWidget);

    await tester.enterText(titleField, 'Project Sync');
    expect(find.text('Project Sync'), findsOneWidget);
  });

  testWidgets('starting sends the trimmed entered title to the bridge',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.enterText(find.byType(TextField), '  Project Sync  ');
    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.requestedTitles, <String?>['Project Sync']);
    expect(find.text('Recording "Project Sync"'), findsOneWidget);
  });

  testWidgets('a blank title is sent as null so Rust uses its fallback',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.enterText(find.byType(TextField), '   ');
    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.requestedTitles, <String?>[null]);
    expect(find.text('Recording'), findsOneWidget);
  });

  testWidgets('submitting the title from the keyboard starts the recording',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.enterText(find.byType(TextField), 'Retro');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();

    expect(recordingBridge.requestedTitles, <String?>['Retro']);
  });

  testWidgets('the title field is locked while recording',
      (WidgetTester tester) async {
    await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true));

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    final TextField titleField =
        tester.widget<TextField>(find.byType(TextField));
    expect(titleField.enabled, isFalse);
  });

  testWidgets('the title field fits a phone and is width-capped on desktop',
      (WidgetTester tester) async {
    addTearDown(tester.view.reset);
    tester.view.devicePixelRatio = 1;

    tester.view.physicalSize = const Size(360, 640);
    await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true));
    expect(tester.takeException(), isNull);
    expect(
        tester.getSize(find.byType(TextField)).width, lessThanOrEqualTo(360));

    tester.view.physicalSize = const Size(1440, 900);
    await tester.pump();
    expect(
        tester.getSize(find.byType(TextField)).width, lessThanOrEqualTo(480));
  });
}
