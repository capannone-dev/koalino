// Tests the Whisper model and OpenAI settings with a fake core.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/transcription_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';
import 'package:note_taker_ui/presentation/transcription_settings_screen.dart';

const WhisperModelInfo _base = WhisperModelInfo(
  fileName: 'ggml-base.bin',
  name: 'base',
  family: 'base',
  sizeBytes: 147951465,
  englishOnly: false,
  onDevice: true,
  removable: false,
  tooLargeForMemory: false,
);

const WhisperModelInfo _small = WhisperModelInfo(
  fileName: 'ggml-small.bin',
  name: 'small',
  family: 'small',
  sizeBytes: 487601967,
  englishOnly: false,
  onDevice: false,
  removable: false,
  tooLargeForMemory: false,
);

const WhisperModelInfo _english = WhisperModelInfo(
  fileName: 'ggml-small.en.bin',
  name: 'small.en',
  family: 'small',
  sizeBytes: 487614201,
  englishOnly: true,
  onDevice: false,
  removable: false,
  tooLargeForMemory: true,
);

final class _FakeBridge implements TranscriptionSettingsBridge {
  String provider = 'local';
  bool keySaved = false;
  bool keychain = true;
  String selected = 'ggml-base.bin';
  WhisperProposal? proposal;
  WhisperDownload downloadState = const WhisperDownload();
  final List<String> calls = <String>[];
  int saveKeyStatus = transcriptionStatusOk;
  String? savedKey;
  WhisperSuggestion? suggestion;
  EfficientModeStatus efficient = const EfficientModeStatus();

  @override
  Future<int> setEfficientGpu({required bool enabled}) async {
    calls.add('efficient $enabled');
    efficient = EfficientModeStatus(
        gpuName: efficient.gpuName,
        enabled: enabled,
        gpuBlocked: efficient.gpuBlocked,
        gpuInUse: efficient.gpuInUse);
    return transcriptionStatusOk;
  }

  @override
  Future<int> retryGpu() async {
    calls.add('retry');
    efficient = EfficientModeStatus(
        gpuName: efficient.gpuName, enabled: efficient.enabled);
    return transcriptionStatusOk;
  }

  @override
  Future<TranscriptionSettings?> read() async => TranscriptionSettings(
        provider: provider,
        openAiModel: 'whisper-1',
        keySaved: keySaved,
        environmentKey: false,
        keychainAvailable: keychain,
        selectedModel: selected,
        availableDiskBytes: 100000000000,
        usingLocalFallback: false,
        models: const <WhisperModelInfo>[_base, _small, _english],
        download: downloadState,
        suggestion: suggestion,
        proposal: proposal,
        efficient: efficient,
        glossary: glossary,
        keepSpeech: SpeechRetention(days: keepSpeechDays),
        afterMeetingPass: afterMeetingPass,
      );

  @override
  Future<int> dismissProposal(String fileName) async {
    calls.add('dismiss $fileName');
    proposal = null;
    return transcriptionStatusOk;
  }

  @override
  Future<int> selectModel(String fileName) async {
    calls.add('select $fileName');
    selected = fileName;
    return transcriptionStatusOk;
  }

  @override
  Future<int> setProvider(String value) async {
    calls.add('provider $value');
    provider = value;
    return transcriptionStatusOk;
  }

  @override
  Future<int> setOpenAiModel(String model) async => transcriptionStatusOk;

  @override
  Future<int> saveKey(String key) async {
    savedKey = key;
    if (saveKeyStatus == transcriptionStatusOk) {
      keySaved = true;
    }
    return saveKeyStatus;
  }

  @override
  Future<int> removeKey() async {
    keySaved = false;
    provider = 'local';
    return transcriptionStatusOk;
  }

  @override
  Future<String> testKey() async => 'invalid';

  @override
  Future<int> download(String fileName) async {
    calls.add('download $fileName');
    downloadState = WhisperDownload(
        fileName: fileName,
        receivedBytes: 10,
        totalBytes: 100,
        state: 'downloading');
    return transcriptionStatusOk;
  }

  @override
  Future<void> cancelDownload() async {
    calls.add('cancel');
    downloadState = const WhisperDownload(state: 'cancelled');
  }

  @override
  Future<int> removeModel(String fileName) async => transcriptionStatusOk;

  /// The glossary as the core holds it.
  List<String> glossary = <String>[];

  /// How many days speech waits to be combined, as the core holds it.
  int keepSpeechDays = 21;

  /// Whether meetings are re-transcribed after Stop, as the core holds it.
  bool afterMeetingPass = true;

  @override
  Future<int> setAfterMeetingPass({required bool enabled}) async {
    calls.add('pass $enabled');
    afterMeetingPass = enabled;
    return transcriptionStatusOk;
  }

  @override
  Future<int> setKeepSpeechDays(int days) async {
    calls.add('keep $days');
    if (days < 1 || days > 60) {
      return transcriptionStatusInvalid;
    }
    keepSpeechDays = days;
    return transcriptionStatusOk;
  }

  @override
  Future<int> setGlossary(List<String> terms) async {
    calls.add('glossary ${terms.join('|')}');
    if (terms.any((String term) => term.length > 60)) {
      return transcriptionStatusInvalid;
    }
    glossary = terms;
    return transcriptionStatusOk;
  }
}

/// Pumps the page over a fake core.
Future<TranscriptionSettingsController> _pump(
    WidgetTester tester, _FakeBridge bridge,
    {String language = 'it'}) async {
  final TranscriptionSettingsController controller =
      TranscriptionSettingsController(
          bridge: bridge, refreshInterval: const Duration(hours: 1));
  addTearDown(controller.dispose);
  await tester.pumpWidget(MaterialApp(
    home: TranscriptionSettingsScreen(
        controller: controller, languageCode: language),
  ));
  await tester.pumpAndSettle();
  return controller;
}

void main() {
  testWidgets('a suggestion on the device is applied with efficient mode',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..selected = 'ggml-base.bin'
      ..efficient = const EfficientModeStatus(gpuName: 'Adreno (TM) 740')
      ..suggestion = const WhisperSuggestion(
          fileName: 'ggml-base.bin', efficient: true, reason: 'Keeps up.');
    await _pump(tester, bridge);
    expect(find.text('Suggested for this device: base, efficient mode'),
        findsOneWidget);
    expect(find.text('Keeps up.'), findsOneWidget);
    await tester.tap(find.byKey(useSuggestionKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('efficient true'));
    expect(find.byKey(useSuggestionKey), findsNothing);
  });

  testWidgets('a proposal on the device is chosen only when asked',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..selected = 'ggml-small.bin'
      ..proposal = const WhisperProposal(
          fileName: 'ggml-base.bin',
          reason: 'small falls behind here.',
          measured: true);
    await _pump(tester, bridge);
    expect(find.text('small falls behind here.'), findsOneWidget);
    expect(bridge.selected, 'ggml-small.bin');
    await tester.ensureVisible(find.byKey(useProposalKey));
    await tester.tap(find.byKey(useProposalKey));
    await tester.pumpAndSettle();
    expect(bridge.selected, 'ggml-base.bin');
  });

  testWidgets('a proposal replaces the device-based suggestion',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..suggestion = const WhisperSuggestion(
          fileName: 'ggml-base.bin', efficient: false, reason: 'Safe choice.')
      ..proposal = const WhisperProposal(
          fileName: 'ggml-small.bin',
          reason: 'Room to spare.',
          measured: false);
    await _pump(tester, bridge);
    expect(find.text('Room to spare.'), findsOneWidget);
    expect(find.text('Safe choice.'), findsNothing);
  });

  testWidgets('a proposal not on the device downloads only when asked',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..proposal = const WhisperProposal(
          fileName: 'ggml-small.bin',
          reason: 'Room to spare.',
          measured: false);
    final TranscriptionSettingsController controller =
        await _pump(tester, bridge);
    await tester.ensureVisible(find.byKey(useProposalKey));
    await tester.tap(find.byKey(useProposalKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('download ggml-small.bin'));
    expect(bridge.selected, 'ggml-base.bin');
    bridge.downloadState = const WhisperDownload(state: 'cancelled');
    await controller.load();
  });

  testWidgets('dismissing a proposal hides it and keeps the model',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..proposal = const WhisperProposal(
          fileName: 'ggml-small.bin',
          reason: 'Room to spare.',
          measured: false);
    await _pump(tester, bridge);
    await tester.ensureVisible(find.byKey(dismissProposalKey));
    await tester.tap(find.byKey(dismissProposalKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('dismiss ggml-small.bin'));
    expect(find.byKey(useProposalKey), findsNothing);
    expect(bridge.selected, 'ggml-base.bin');
  });

  testWidgets('a suggestion not on the device downloads only when asked',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..suggestion = const WhisperSuggestion(
          fileName: 'ggml-small.bin', efficient: false, reason: 'Keeps up.');
    final TranscriptionSettingsController controller =
        await _pump(tester, bridge);
    expect(bridge.calls.where((String call) => call.startsWith('download')),
        isEmpty);
    expect(bridge.selected, 'ggml-base.bin');
    await tester.tap(find.byKey(useSuggestionKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('download ggml-small.bin'));
    expect(bridge.selected, 'ggml-base.bin');
    bridge.downloadState = const WhisperDownload(state: 'cancelled');
    await controller.load();
  });

  testWidgets('efficient mode is offered only with a supported GPU',
      (WidgetTester tester) async {
    await _pump(tester, _FakeBridge());
    expect(find.byKey(efficientModeKey), findsNothing);
  });

  testWidgets('efficient mode can be turned on and off',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..efficient = const EfficientModeStatus(gpuName: 'Adreno (TM) 740');
    await _pump(tester, bridge);
    await tester.tap(find.byKey(efficientModeKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('efficient true'));
    await tester.tap(find.byKey(efficientModeKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('efficient false'));
  });

  testWidgets('a GPU that crashed says so and can be tried again',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..efficient = const EfficientModeStatus(
          gpuName: 'Adreno (TM) 740', enabled: true, gpuBlocked: true);
    await _pump(tester, bridge);
    expect(find.textContaining('stopped the app'), findsOneWidget);
    await tester.tap(find.text('Try the graphics chip again'));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('retry'));
    expect(find.textContaining('stopped the app'), findsNothing);
  });

  testWidgets('saving a key keeps recordings local and clears the field',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    await tester.enterText(find.byKey(openAiKeyFieldKey), 'sk-secret');
    await tester.tap(find.text('Save key'));
    await tester.pumpAndSettle();
    expect(bridge.savedKey, 'sk-secret');
    expect(bridge.provider, 'local');
    expect(find.text('sk-secret'), findsNothing);
    expect(find.text('An OpenAI key is saved.'), findsOneWidget);
  });

  testWidgets('choosing OpenAI needs confirmation that audio leaves the device',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()..keySaved = true;
    await _pump(tester, bridge);
    await tester.tap(find.byKey(providerSelectorKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('OpenAI').last);
    await tester.pumpAndSettle();
    expect(find.text('Send recordings to OpenAI?'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(bridge.provider, 'local');

    await tester.tap(find.byKey(providerSelectorKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('OpenAI').last);
    await tester.pumpAndSettle();
    await tester.tap(find.text('Send to OpenAI'));
    await tester.pumpAndSettle();
    expect(bridge.provider, 'openai');
  });

  testWidgets('without a keychain the key cannot be typed',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()..keychain = false;
    await _pump(tester, bridge);
    expect(find.byKey(openAiKeyFieldKey), findsNothing);
    expect(find.textContaining('no keychain'), findsOneWidget);
  });

  testWidgets('a model on the device can be chosen and others downloaded',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    expect(find.byTooltip('Download small'), findsOneWidget);
    await tester.tap(find.byTooltip('Download small'));
    await tester.pump();
    await tester.pump();
    expect(bridge.calls, contains('download ggml-small.bin'));
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    await tester.tap(find.byTooltip('Cancel download'));
    await tester.pump();
    await tester.pump();
    expect(bridge.calls, contains('cancel'));
    expect(find.text('Download cancelled.'), findsOneWidget);
  });

  testWidgets('a model too large asks first, and English-only is flagged',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byTooltip('Download small.en'), 100,
        scrollable: find.byType(Scrollable).first);
    expect(find.textContaining('doesn\'t match your language'), findsOneWidget);
    await tester.ensureVisible(find.byTooltip('Download small.en'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Download small.en'));
    await tester.pumpAndSettle();
    expect(find.text('This model may be too large'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(bridge.calls, isNot(contains('download ggml-small.en.bin')));
  });

  test('a failed download explains that the current model still works',
      () async {
    final _FakeBridge bridge = _FakeBridge();
    final TranscriptionSettingsController controller =
        TranscriptionSettingsController(
            bridge: bridge, refreshInterval: const Duration(hours: 1));
    addTearDown(controller.dispose);
    await controller.download('ggml-small.bin');
    bridge.downloadState = const WhisperDownload(
        state: 'failed', error: 'corrupted', fileName: 'ggml-small.bin');
    await controller.load();
    expect(controller.message, contains('failed its check'));
    expect(controller.message, contains('still works'));
  });

  test('a refused key test reads plainly', () async {
    final TranscriptionSettingsController controller =
        TranscriptionSettingsController(bridge: _FakeBridge());
    addTearDown(controller.dispose);
    await controller.testKey();
    expect(controller.message, contains('refused'));
  });

  testWidgets('the glossary shows the saved terms and saves one per line',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()..glossary = <String>['OKR'];
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(glossaryFieldKey), 200,
        scrollable: find.byType(Scrollable).first);
    expect(find.text('OKR'), findsOneWidget);

    await tester.enterText(
        find.byKey(glossaryFieldKey), 'Marco Rossi\n\n  Kubernetes \nOKR');
    await tester.tap(find.byKey(glossarySaveKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, contains('glossary Marco Rossi|Kubernetes|OKR'));
    expect(bridge.glossary, <String>['Marco Rossi', 'Kubernetes', 'OKR']);
    expect(find.textContaining('next recording'), findsWidgets);
  });

  testWidgets('the speech retention window is chosen in days',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(keepSpeechKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.ensureVisible(find.byKey(keepSpeechKey));
    await tester.pumpAndSettle();
    expect(find.text('21 days'), findsOneWidget);

    await tester.tap(find.byKey(keepSpeechKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('7 days').last);
    await tester.pumpAndSettle();

    expect(bridge.calls, contains('keep 7'));
    expect(bridge.keepSpeechDays, 7);
    await tester.scrollUntilVisible(find.byKey(keepSpeechKey), 200,
        scrollable: find.byType(Scrollable).first);
    expect(find.text('7 days'), findsOneWidget,
        reason: 'the menu shows the saved window');
  });

  testWidgets('the after-meeting pass is on by default and can be turned off',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(afterMeetingPassKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.pumpAndSettle();
    expect(tester.widget<SwitchListTile>(find.byKey(afterMeetingPassKey)).value,
        isTrue);

    await tester.tap(find.byKey(afterMeetingPassKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, contains('pass false'));
    await tester.scrollUntilVisible(find.byKey(afterMeetingPassKey), 200,
        scrollable: find.byType(Scrollable).first);
    expect(tester.widget<SwitchListTile>(find.byKey(afterMeetingPassKey)).value,
        isFalse,
        reason: 'the switch shows the saved choice');
  });

  test('settings from an older core keep the after-meeting pass on', () {
    expect(
        const TranscriptionSettings(
          provider: 'local',
          openAiModel: 'whisper-1',
          keySaved: false,
          environmentKey: false,
          keychainAvailable: true,
          selectedModel: null,
          availableDiskBytes: null,
          usingLocalFallback: false,
          models: <WhisperModelInfo>[],
          download: WhisperDownload(state: 'idle'),
        ).afterMeetingPass,
        isTrue);
  });

  test('the retention window is read from the core with its bounds', () {
    final SpeechRetention retention = SpeechRetention.fromJson(
        <String, Object?>{'days': 9, 'min_days': 1, 'max_days': 60});
    expect(retention.days, 9);
    expect(retention.choices, contains(9),
        reason: 'a window set elsewhere is offered');
    expect(retention.choices.first, 1);
    expect(retention.choices.last, 60);
  });

  testWidgets('a glossary the core refuses says why and is not lost',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(glossaryFieldKey), 200,
        scrollable: find.byType(Scrollable).first);
    final String longTerm = 'x' * 61;

    await tester.enterText(find.byKey(glossaryFieldKey), longTerm);
    await tester.tap(find.byKey(glossarySaveKey));
    await tester.pumpAndSettle();

    expect(find.textContaining('60 characters'), findsWidgets);
    expect(find.text(longTerm), findsOneWidget,
        reason: 'the refused text stays');
    expect(bridge.glossary, isEmpty);
  });
}
