// Specifies choosing the transcription language: the default comes from Settings and can be AUTO,
// the Record screen starts on it, a choice works before and during a recording, it lasts until
// that recording ends, and a refusal keeps the old language.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/recording_notifier.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'fake_language_bridge.dart';
import 'fake_log_bridge.dart';
import 'recording_screen_test.dart' show FakeRecordingBridge;

/// A recording controller over a fake core.
RecordingController _recording() => RecordingController(
      recordingBridge: FakeRecordingBridge(startResult: true),
      microphonePermission: const GrantedMicrophonePermission(),
      recordingNotifier: const NoRecordingNotifier(),
    );

/// A loaded language controller.
Future<TranscriptionLanguageController> _loaded(
  FakeLanguageBridge bridge, {
  RecordingController? recording,
}) async {
  final TranscriptionLanguageController controller =
      TranscriptionLanguageController(
    languageBridge: bridge,
    recordingController: recording,
  );
  await controller.load();
  return controller;
}

void main() {
  test('offers AUTO first, then languages by name', () async {
    final TranscriptionLanguageController controller =
        await _loaded(FakeLanguageBridge());

    expect(controller.choices.first.code, 'auto');
    expect(controller.choices.first.label, 'Auto');
    expect(
        controller.choices
            .any((LanguageChoice c) => c.code == 'it' && c.label == 'Italian'),
        isTrue);
    final List<String> names = controller.choices
        .skip(1)
        .map((LanguageChoice c) => c.label)
        .toList(growable: false);
    expect(names, List<String>.of(names)..sort());
    expect(controller.choices.length, 101);
  });

  test('the recording language starts on the default and follows it', () async {
    final FakeLanguageBridge bridge = FakeLanguageBridge(storedDefault: 'it');
    final TranscriptionLanguageController controller = await _loaded(bridge);

    expect(controller.defaultLanguage, 'it');
    expect(controller.recordingLanguage, 'it');

    await controller.chooseDefault('auto');
    expect(bridge.storedDefault, 'auto');
    expect(controller.recordingLanguage, 'auto');
    expect(bridge.recordingLanguages, isEmpty);
  });

  test('a choice for the recording leaves the default alone', () async {
    final FakeLanguageBridge bridge = FakeLanguageBridge();
    final TranscriptionLanguageController controller = await _loaded(bridge);

    await controller.chooseForRecording('en');

    expect(controller.recordingLanguage, 'en');
    expect(controller.defaultLanguage, 'auto');
    expect(bridge.storedDefault, 'auto');
    expect(bridge.recordingLanguages, <String>['en']);
  });

  test('a refused change keeps the old language and says so', () async {
    final FakeLanguageBridge bridge = FakeLanguageBridge(refuses: true);
    final TranscriptionLanguageController controller = await _loaded(bridge);

    await controller.chooseForRecording('en');
    expect(controller.recordingLanguage, 'auto');
    expect(controller.message, isNotNull);

    await controller.chooseDefault('it');
    expect(controller.defaultLanguage, 'auto');
    expect(controller.message, isNotNull);
  });

  test('the choice lasts until the recording ends, then the default returns',
      () async {
    final RecordingController recording = _recording();
    final TranscriptionLanguageController controller = await _loaded(
        FakeLanguageBridge(storedDefault: 'it'),
        recording: recording);

    await controller.chooseForRecording('en');
    await recording.startRecording();
    expect(recording.state, RecordingUiState.recording);
    expect(controller.recordingLanguage, 'en');

    await recording.stopRecording();
    expect(recording.state, RecordingUiState.idle);
    expect(controller.recordingLanguage, 'it');
  });

  test('changing the default during a recording does not change that recording',
      () async {
    final RecordingController recording = _recording();
    final TranscriptionLanguageController controller = await _loaded(
        FakeLanguageBridge(storedDefault: 'it'),
        recording: recording);

    await recording.startRecording();
    await controller.chooseDefault('de');
    expect(controller.recordingLanguage, 'it');

    await recording.stopRecording();
    expect(controller.recordingLanguage, 'de');
  });

  testWidgets('the Record screen changes the language before and during',
      (WidgetTester tester) async {
    tester.view.physicalSize = const Size(1200, 1600);
    tester.view.devicePixelRatio = 1;
    addTearDown(tester.view.reset);
    final RecordingController recording = _recording();
    final FakeLanguageBridge bridge = FakeLanguageBridge();
    final TranscriptionLanguageController controller =
        await _loaded(bridge, recording: recording);
    await tester.pumpWidget(MaterialApp(
      home: RecordingScreen(
        recordingController: recording,
        languageController: controller,
      ),
    ));

    Future<void> choose(String label) async {
      await tester.tap(find.byKey(recordingLanguageSelectorKey));
      await tester.pumpAndSettle();
      await tester.tap(find.text(label).last);
      await tester.pumpAndSettle();
    }

    expect(find.text('Auto'), findsOneWidget);
    await choose('Afrikaans');
    expect(bridge.recordingLanguages, <String>['af']);

    await recording.startRecording();
    await tester.pump();
    await tester.ensureVisible(find.byKey(recordingLanguageSelectorKey));
    await choose('Albanian');
    expect(bridge.recordingLanguages, <String>['af', 'sq']);
  });

  testWidgets('Settings stores the default language',
      (WidgetTester tester) async {
    final FakeLanguageBridge bridge = FakeLanguageBridge();
    final TranscriptionLanguageController controller = await _loaded(bridge);
    await tester.pumpWidget(MaterialApp(
      home: SettingsScreen(
        logSettingsController: LogSettingsController(
          logBridge: FakeLogBridge(),
          exportSaver: FakeDesktopSaver(chosenPath: null),
          sharedDirectory: () async => Directory.systemTemp,
        ),
        languageController: controller,
      ),
    ));

    expect(find.text('Default language'), findsOneWidget);
    await tester.tap(find.byKey(defaultLanguageSelectorKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Afrikaans').last);
    await tester.pumpAndSettle();

    expect(bridge.storedDefault, 'af');
  });
}
