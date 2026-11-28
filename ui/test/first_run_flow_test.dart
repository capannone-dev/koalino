// Specifies the first-run flow (#202): at most three skippable screens on first launch (what
// Koalino does and its privacy promise; the microphone with its reason where the system asks;
// the transcription language with the recommended model), never shown again.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/seen_tips_store.dart';
import 'package:note_taker_ui/presentation/first_run_flow.dart';

import 'fake_language_bridge.dart';

/// A permission whose answer the test sets, counting the requests.
final class ScriptedMicrophonePermission implements MicrophonePermission {
  ScriptedMicrophonePermission({required this.grants});

  final bool grants;
  int requests = 0;

  @override
  Future<bool> ensureGranted() async {
    requests++;
    return grants;
  }
}

/// Pumps the flow over a placeholder app on a phone-sized screen.
Future<void> pumpFlow(
  WidgetTester tester, {
  required SeenTipsStore store,
  MicrophonePermission? microphone,
  TranscriptionLanguageController? language,
}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(390, 844);
  addTearDown(tester.view.reset);
  await tester.pumpWidget(MaterialApp(
    home: FirstRunFlow(
      store: store,
      microphonePermission: microphone,
      languageController: language,
      modelSettingsBuilder: (BuildContext context) =>
          const Scaffold(body: Text('Model settings')),
      child: const Scaffold(body: Text('The app')),
    ),
  ));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('three screens, the privacy promise first; the end opens the app',
      (WidgetTester tester) async {
    final MemorySeenTipsStore store = MemorySeenTipsStore();
    final ScriptedMicrophonePermission microphone =
        ScriptedMicrophonePermission(grants: true);
    final FakeLanguageBridge bridge = FakeLanguageBridge();
    await pumpFlow(tester,
        store: store,
        microphone: microphone,
        language: TranscriptionLanguageController(languageBridge: bridge));

    expect(find.text('Welcome to Koalino'), findsOneWidget);
    expect(find.textContaining('Private by design'), findsOneWidget);
    expect(find.text('1 of 3'), findsOneWidget);
    expect(find.text('The app'), findsNothing);

    await tester.tap(find.byKey(firstRunNextKey));
    await tester.pumpAndSettle();
    expect(find.textContaining('needs the microphone'), findsOneWidget);
    expect(microphone.requests, 0, reason: 'asked only when the owner taps');
    await tester.tap(find.text('Allow microphone'));
    await tester.pumpAndSettle();
    expect(microphone.requests, 1);
    expect(find.text('Microphone allowed.'), findsOneWidget);

    await tester.tap(find.byKey(firstRunNextKey));
    await tester.pumpAndSettle();
    expect(find.textContaining('Automatic (recommended)'), findsOneWidget);
    final Finder languageField = find.descendant(
        of: find.byKey(const ValueKey<String>('first-run-language')),
        matching: find.byType(TextField));
    await tester.tap(languageField);
    await tester.pumpAndSettle();
    await tester.enterText(languageField, 'Ital');
    await tester.pumpAndSettle();
    await tester.tap(find.text('Italian').last);
    await tester.pumpAndSettle();
    expect(bridge.storedDefault, 'it');

    await tester.tap(find.text('Start using Koalino'));
    await tester.pumpAndSettle();
    expect(find.text('The app'), findsOneWidget);
    expect(await store.seen(firstRunTipName), isTrue);
  });

  testWidgets('a refused microphone says it will be asked again',
      (WidgetTester tester) async {
    await pumpFlow(tester,
        store: MemorySeenTipsStore(),
        microphone: ScriptedMicrophonePermission(grants: false));
    await tester.tap(find.byKey(firstRunNextKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Allow microphone'));
    await tester.pumpAndSettle();
    expect(find.textContaining('asks again when you first record'),
        findsOneWidget);
    expect(find.text('2 of 2'), findsOneWidget);
  });

  testWidgets('Skip opens the app at once and for good',
      (WidgetTester tester) async {
    final MemorySeenTipsStore store = MemorySeenTipsStore();
    await pumpFlow(tester, store: store);
    expect(find.text('1 of 1'), findsOneWidget,
        reason: 'desktops without a prompt or language skip those screens');

    await tester.tap(find.byKey(firstRunSkipKey));
    await tester.pumpAndSettle();
    expect(find.text('The app'), findsOneWidget);
    expect(await store.seen(firstRunTipName), isTrue);
  });

  testWidgets('never shown again once seen', (WidgetTester tester) async {
    await pumpFlow(tester,
        store: MemorySeenTipsStore(<String>[firstRunTipName]));
    expect(find.text('The app'), findsOneWidget);
    expect(find.text('Welcome to Koalino'), findsNothing);
  });

  testWidgets('Choose another model opens the model settings',
      (WidgetTester tester) async {
    await pumpFlow(tester,
        store: MemorySeenTipsStore(),
        language: TranscriptionLanguageController(
            languageBridge: FakeLanguageBridge()));
    await tester.tap(find.byKey(firstRunNextKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Choose another model'));
    await tester.pumpAndSettle();
    expect(find.text('Model settings'), findsOneWidget);
  });
}
