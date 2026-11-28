// Tests the Whisper model settings with a fake core.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/transcription_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/npu_download.dart';
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
  /// The hand-picked model; `null` is Automatic.
  String? selected = 'ggml-base.bin';

  /// What Automatic runs, and the suggested model it waits for, if any.
  String? automaticFile = 'ggml-base.bin';
  String? automaticWaiting;
  bool automaticEfficient = false;

  /// The speed check's sentence (#109), if Automatic's model was timed here.
  String? automaticSpeedCheck;

  @override
  Future<int> useAutomaticModel() async {
    calls.add('automatic');
    selected = null;
    return transcriptionStatusOk;
  }

  WhisperProposal? proposal;

  /// Free space reported to the page.
  int? availableDiskBytes = 100000000000;
  WhisperDownload downloadState = const WhisperDownload();
  final List<String> calls = <String>[];
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
        selectedModel: selected,
        availableDiskBytes: availableDiskBytes,
        models: const <WhisperModelInfo>[_base, _small, _english],
        download: downloadState,
        suggestion: suggestion,
        proposal: proposal,
        efficient: efficient,
        glossary: glossary,
        keepSpeech: SpeechRetention(days: keepSpeechDays),
        afterMeetingPass: afterMeetingPass,
        afterMeetingPassModel: afterMeetingPassModel,
        autoDownload: autoDownload,
        computerSound: ComputerSoundSetting(
            available: computerSoundAvailable, enabled: computerSound),
        npu: npu,
        automatic: AutomaticModelStatus(
            enabled: selected == null,
            fileName: automaticFile,
            efficient: automaticEfficient,
            waitingForDownload: automaticWaiting,
            speedCheck: automaticSpeedCheck),
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

  /// The pass's own model, or null for the live one (#87).
  String? afterMeetingPassModel;

  @override
  Future<int> setAfterMeetingPassModel(String? fileName) async {
    calls.add('pass model $fileName');
    afterMeetingPassModel = fileName;
    return transcriptionStatusOk;
  }

  /// Whether this build can record the computer's sound, and whether it does (#108).
  bool computerSoundAvailable = false;
  bool computerSound = true;

  @override
  Future<int> setComputerSound({required bool enabled}) async {
    calls.add('computer sound $enabled');
    computerSound = enabled;
    return transcriptionStatusOk;
  }

  /// Whether Automatic's model downloads by itself (#189).
  bool autoDownload = true;

  @override
  Future<int> setAutoDownload({required bool enabled}) async {
    calls.add('auto download $enabled');
    autoDownload = enabled;
    return transcriptionStatusOk;
  }

  @override
  Future<int> setAfterMeetingPass({required bool enabled}) async {
    calls.add('pass $enabled');
    afterMeetingPass = enabled;
    return transcriptionStatusOk;
  }

  /// The NPU's state as the core reports it (#75).
  NpuStatus npu = const NpuStatus();

  @override
  Future<int> setNpuWhisper({required bool enabled}) async {
    calls.add('npu $enabled');
    if (enabled && !npu.installed) {
      return transcriptionStatusInvalid;
    }
    npu = npu.copyWith(enabled: enabled);
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

/// The Snapdragon 8 Gen 3's build, as the core reports it (#183).
const NpuBuild _eightGen3 = NpuBuild(
  chip: 'Snapdragon 8 Gen 3',
  url: 'https://example.org/8gen3.zip',
  sha256: '575cafbe',
  sizeBytes: 285196969,
  entryPrefix: 'whisper/',
  hexagon: 75,
);

/// Downloads the NPU files on a script: each `progress` call returns the next state.
final class _FakeNpuDownloader implements NpuDownloader {
  _FakeNpuDownloader(this.bridge);

  final _FakeBridge bridge;
  final List<String> calls = <String>[];
  List<NpuDownloadProgress> script = <NpuDownloadProgress>[];

  @override
  Future<void> download(String directory, NpuBuild build) async {
    calls.add('download $directory V${build.hexagon}');
  }

  @override
  Future<NpuDownloadProgress> progress() async {
    if (!calls.any((String call) => call.startsWith('download'))) {
      return const NpuDownloadProgress(state: 'idle');
    }
    final NpuDownloadProgress next = script.isEmpty
        ? const NpuDownloadProgress(state: 'idle')
        : script.removeAt(0);
    if (next.state == 'done') {
      bridge.npu = bridge.npu.copyWith(installed: true);
    }
    return next;
  }

  @override
  Future<void> cancel() async => calls.add('cancel');

  @override
  Future<bool> remove(String directory) async {
    calls.add('remove $directory');
    bridge.npu = NpuStatus(
        available: true,
        supported: true,
        directory: bridge.npu.directory,
        build: bridge.npu.build);
    return true;
  }
}

/// Pumps the page over a fake core.
Future<TranscriptionSettingsController> _pump(
    WidgetTester tester, _FakeBridge bridge,
    {String language = 'it',
    NpuDownloader? npuDownloader,
    bool bundlesWhisperModel = true}) async {
  final TranscriptionSettingsController controller =
      TranscriptionSettingsController(
          bridge: bridge,
          refreshInterval: const Duration(hours: 1),
          npuDownloader: npuDownloader ?? const NoNpuDownloader(),
          npuRefreshInterval: const Duration(milliseconds: 10));
  addTearDown(controller.dispose);
  await tester.pumpWidget(MaterialApp(
    home: TranscriptionSettingsScreen(
        controller: controller,
        languageCode: language,
        bundlesWhisperModel: bundlesWhisperModel),
  ));
  await tester.pumpAndSettle();
  return controller;
}

/// A catalog row for the list-splitting tests.
WhisperModelInfo _row(String name,
        {bool onDevice = false, bool tooLarge = false}) =>
    WhisperModelInfo(
      fileName: 'ggml-$name.bin',
      name: name,
      family: name.split('-q').first.replaceAll('.en', ''),
      sizeBytes: 1000,
      englishOnly: name.contains('.en'),
      onDevice: onDevice,
      removable: onDevice,
      tooLargeForMemory: tooLarge,
    );

void main() {
  test('one row per size is shown; variants wait under More models (#75)', () {
    final List<WhisperModelInfo> catalog = <WhisperModelInfo>[
      _row('tiny', onDevice: true),
      _row('tiny-q5_1'),
      _row('tiny.en'),
      _row('base', onDevice: true),
      _row('base-q8_0', onDevice: true),
      _row('small'),
      _row('small.en-q5_1'),
      _row('medium', tooLarge: true),
      _row('large-v2'),
      _row('large-v3'),
      _row('large-v3-turbo', tooLarge: true),
      _row('large-v3-turbo-q5_0'),
    ];

    final ModelRows rows =
        splitModelRows(catalog, pinned: <String>{'ggml-small.en-q5_1.bin'});

    expect(rows.main.map((WhisperModelInfo row) => row.name), <String>[
      'tiny',
      'base',
      'base-q8_0',
      'small',
      'small.en-q5_1',
    ]);
    expect(rows.more.map((WhisperModelInfo row) => row.name), <String>[
      'tiny-q5_1',
      'tiny.en',
      'medium',
      'large-v2',
      'large-v3',
      'large-v3-turbo',
      'large-v3-turbo-q5_0',
    ]);
  });

  testWidgets('base is said to ship with the app when it does',
      (WidgetTester tester) async {
    await _pump(tester, _FakeBridge());
    expect(find.textContaining('The app ships with this one'), findsWidgets);
  });

  testWidgets('a build without a bundled model does not claim one (#163)',
      (WidgetTester tester) async {
    await _pump(tester, _FakeBridge(), bundlesWhisperModel: false);
    expect(find.textContaining('The app ships with this one'), findsNothing);
    expect(find.textContaining('Fast, many mistakes'), findsWidgets);
  });

  testWidgets('More models opens the variants and says how many there are',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    expect(find.byTooltip('Download small.en'), findsNothing);
    await tester.scrollUntilVisible(find.byKey(moreModelsKey), 100,
        scrollable: find.byType(Scrollable).first);
    expect(find.text('More models (1)'), findsOneWidget);
    await tester.tap(find.byKey(moreModelsKey));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Download small.en'), findsOneWidget);
  });

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
    await tester.ensureVisible(find.text('Try the graphics chip again'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Try the graphics chip again'));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('retry'));
    expect(find.textContaining('stopped the app'), findsNothing);
  });

  testWidgets('the page offers no API key or cloud provider (#117)',
      (WidgetTester tester) async {
    await _pump(tester, _FakeBridge());
    expect(find.textContaining('OpenAI'), findsNothing);
    expect(find.textContaining('API key'), findsNothing);
  });

  testWidgets('a model on the device can be chosen and others downloaded',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    expect(find.byTooltip('Download small'), findsOneWidget);
    await tester.ensureVisible(find.byTooltip('Download small'));
    await tester.pumpAndSettle();
    await tester.tap(find.byTooltip('Download small'));
    await tester.pump();
    await tester.pump();
    expect(bridge.calls, contains('download ggml-small.bin'));
    expect(find.byType(LinearProgressIndicator), findsOneWidget);
    await tester.tap(find.byTooltip('Cancel download'));
    await tester.pump();
    await tester.pump();
    expect(bridge.calls, contains('cancel'));
    await tester.scrollUntilVisible(find.text('Download cancelled.'), -200,
        scrollable: find.byType(Scrollable).first);
    expect(find.text('Download cancelled.'), findsOneWidget);
  });

  testWidgets('a model too large asks first, and English-only is flagged',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(moreModelsKey), 100,
        scrollable: find.byType(Scrollable).first);
    await tester.tap(find.byKey(moreModelsKey));
    await tester.pumpAndSettle();
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

  testWidgets('Automatic names the model it runs and is chosen with a tap',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..automaticFile = 'ggml-small.bin'
      ..automaticEfficient = true
      ..efficient = const EfficientModeStatus(gpuName: 'Adreno (TM) 740')
      ..suggestion = const WhisperSuggestion(
          fileName: 'ggml-small.bin',
          efficient: true,
          reason: 'Fewer mistakes.');
    await _pump(tester, bridge);
    expect(find.byKey(efficientModeKey), findsOneWidget,
        reason: 'a hand-picked model keeps its own switch');

    await tester.tap(find.byKey(automaticModelKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, contains('automatic'));
    expect(find.text('Automatic: small, efficient mode'), findsOneWidget);
    expect(find.text('Fewer mistakes.'), findsOneWidget);
    expect(find.byKey(efficientModeKey), findsNothing,
        reason: 'Automatic decides efficient mode');
    expect(find.byKey(useSuggestionKey), findsNothing,
        reason: 'Automatic already follows the suggestion');
    expect(
        find.descendant(
            of: find.byKey(automaticModelKey),
            matching: find.byIcon(Icons.radio_button_checked)),
        findsOneWidget);
  });

  testWidgets('Automatic shows how its model ran on this device (#109)',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..selected = null
      ..automaticSpeedCheck =
          'Checked on this device: 0.3× real time, so it keeps up with recordings.'
      ..suggestion = const WhisperSuggestion(
          fileName: 'ggml-base.bin', efficient: false, reason: 'Fits.');
    await _pump(tester, bridge);

    expect(
        find.text('Fits. Checked on this device: 0.3× real time, so it keeps '
            'up with recordings.'),
        findsOneWidget);
    expect(
        AutomaticModelStatus.fromJson(<String, Object?>{
          'enabled': true,
          'speed_check': 'Checked on this device: 0.3× real time.',
        }).speedCheck,
        'Checked on this device: 0.3× real time.');
  });

  testWidgets(
      'Automatic waiting for a download uses the shipped model and asks',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..selected = null
      ..automaticWaiting = 'ggml-small.bin'
      ..suggestion = const WhisperSuggestion(
          fileName: 'ggml-small.bin',
          efficient: false,
          reason: 'Fewer mistakes.');
    final TranscriptionSettingsController controller =
        await _pump(tester, bridge);

    expect(find.text('Automatic: base'), findsOneWidget);
    expect(find.textContaining('until small is downloaded'), findsOneWidget);
    expect(bridge.calls, isEmpty, reason: 'nothing downloads by itself');

    await tester.tap(find.byKey(automaticDownloadKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, <String>['download ggml-small.bin'],
        reason: 'the download leaves Automatic on');
    bridge.downloadState = const WhisperDownload(state: 'cancelled');
    await controller.load();
  });

  testWidgets('Automatic warns ahead when its download does not fit (#181)',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..selected = null
      ..automaticWaiting = 'ggml-small.bin'
      ..availableDiskBytes = 150000000
      ..suggestion = const WhisperSuggestion(
          fileName: 'ggml-small.bin',
          efficient: false,
          reason: 'Fewer mistakes.');
    await _pump(tester, bridge);

    final Text automaticSubtitle = tester.widget<Text>(find.descendant(
        of: find.byKey(automaticModelKey),
        matching: find.textContaining('until small is downloaded')));
    expect(automaticSubtitle.data,
        contains("There isn't enough free space to download it yet."));
  });

  test('settings from a core without Automatic show the hand-picked model', () {
    expect(const AutomaticModelStatus().enabled, isFalse);
    expect(
        AutomaticModelStatus.fromJson(<String, Object?>{
          'enabled': true,
          'file_name': 'ggml-small.bin',
          'efficient': true,
          'waiting_for_download': null,
        }).fileName,
        'ggml-small.bin');
  });

  testWidgets(
      'downloading Automatic\'s model by itself is on by default and can be turned off',
      (WidgetTester tester) async {
    // Shown only while Automatic chooses the model.
    final _FakeBridge bridge = _FakeBridge()..selected = null;
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(autoDownloadKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.pumpAndSettle();
    expect(tester.widget<SwitchListTile>(find.byKey(autoDownloadKey)).value,
        isTrue);

    await tester.tap(find.byKey(autoDownloadKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, contains('auto download false'));
    await tester.scrollUntilVisible(find.byKey(autoDownloadKey), 200,
        scrollable: find.byType(Scrollable).first);
    expect(tester.widget<SwitchListTile>(find.byKey(autoDownloadKey)).value,
        isFalse);
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

  testWidgets(
      'the pass model lists models on this device and can return to the live one',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(afterMeetingPassModelKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.pumpAndSettle();
    expect(find.text('Same as for recording'), findsOneWidget);

    await tester.tap(find.byKey(afterMeetingPassModelKey));
    await tester.pumpAndSettle();
    // Only `base` is on this device; `small` would need a download first.
    expect(find.text('small'), findsNothing);
    await tester.tap(find.text('base').last);
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('pass model ggml-base.bin'));

    await tester.scrollUntilVisible(find.byKey(afterMeetingPassModelKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.tap(find.byKey(afterMeetingPassModelKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Same as for recording').last);
    await tester.pumpAndSettle();
    expect(bridge.calls.last, 'pass model null');
  });

  testWidgets('the pass model is hidden while the pass is off',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()..afterMeetingPass = false;
    await _pump(tester, bridge);
    expect(find.byKey(afterMeetingPassModelKey), findsNothing);
  });

  test('settings from an older core keep the after-meeting pass on', () {
    expect(
        const TranscriptionSettings(
          selectedModel: null,
          availableDiskBytes: null,
          models: <WhisperModelInfo>[],
          download: WhisperDownload(state: 'idle'),
        ).afterMeetingPass,
        isTrue);
  });

  testWidgets('the computer sound switch shows only where it can be recorded',
      (WidgetTester tester) async {
    await _pump(tester, _FakeBridge());
    expect(find.byKey(computerSoundKey), findsNothing,
        reason: 'a phone cannot record the computer');
  });

  testWidgets('the computer sound is recorded until turned off',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()..computerSoundAvailable = true;
    await _pump(tester, bridge);
    await tester.scrollUntilVisible(find.byKey(computerSoundKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.pumpAndSettle();
    expect(tester.widget<SwitchListTile>(find.byKey(computerSoundKey)).value,
        isTrue);

    await tester.tap(find.byKey(computerSoundKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('computer sound false'));
  });

  test('the computer sound setting is read from the core', () {
    final ComputerSoundSetting setting = ComputerSoundSetting.fromJson(
        <String, Object?>{'available': true, 'enabled': false});
    expect(setting.available, isTrue);
    expect(setting.enabled, isFalse);
    expect(const ComputerSoundSetting().available, isFalse,
        reason: 'an older core offers nothing');
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

  test('the NPU status is read from the core (#75)', () {
    final NpuStatus status = NpuStatus.fromJson(<String, Object?>{
      'available': true,
      'supported': true,
      'directory': '/data/npu',
      'installed': false,
      'enabled': false,
      'last_open': 'failed',
      'build': <String, Object?>{
        'chip': 'Snapdragon 8 Gen 3',
        'url': 'https://example.org/model.zip',
        'sha256': 'ab',
        'size_bytes': 285196969,
        'entry_prefix': 'model/',
        'hexagon': 75,
      },
    });
    expect(status.supported, isTrue);
    expect(status.build?.hexagon, 75);
    expect(status.build?.sizeBytes, 285196969);
    expect(status.lastOpen, 'failed');
    expect(status.directory, '/data/npu');
    expect(status.installed, isFalse);
    expect(const NpuStatus().supported, isFalse,
        reason: 'an older core offers nothing');
    final NpuDownloadProgress progress =
        NpuDownloadProgress.fromMap(<Object?, Object?>{
      'state': 'downloading',
      'receivedBytes': 5,
      'totalBytes': 10,
      'error': null,
    });
    expect(progress.fraction, 0.5);
  });

  testWidgets('the NPU is offered only on a phone that can run it',
      (WidgetTester tester) async {
    await _pump(tester, _FakeBridge());
    expect(find.byKey(npuDownloadKey), findsNothing);
    expect(find.byKey(npuSwitchKey), findsNothing);
  });

  testWidgets(
      'the NPU files download after accepting Qualcomm\'s terms, then the NPU is on',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..npu = const NpuStatus(
          available: true,
          supported: true,
          directory: '/data/npu',
          build: _eightGen3);
    final _FakeNpuDownloader downloader = _FakeNpuDownloader(bridge)
      ..script = <NpuDownloadProgress>[
        const NpuDownloadProgress(
            state: 'downloading', receivedBytes: 50, totalBytes: 100),
        const NpuDownloadProgress(
            state: 'done', receivedBytes: 100, totalBytes: 100),
      ];
    final TranscriptionSettingsController controller =
        await _pump(tester, bridge, npuDownloader: downloader);
    await tester.scrollUntilVisible(find.byKey(npuDownloadKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(npuDownloadKey));
    await tester.pumpAndSettle();
    expect(find.textContaining('AI Stack License'), findsWidgets);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(downloader.calls, isEmpty, reason: 'nothing without consent');

    await tester.tap(find.byKey(npuDownloadKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Accept and download'));
    await tester.pump();
    expect(downloader.calls, <String>['download /data/npu V75'],
        reason: 'the build and library pair for this chip (#183)');
    await tester.pump(const Duration(milliseconds: 10));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 10));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('npu true'),
        reason: 'turned on once the files are complete');
    expect(controller.message, contains('transcribe on the NPU'));
    expect(
        tester.widget<SwitchListTile>(find.byKey(npuSwitchKey)).value, isTrue);
  });

  testWidgets('a failed NPU download says why and leaves the NPU off',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..npu = const NpuStatus(
          available: true,
          supported: true,
          directory: '/data/npu',
          build: _eightGen3);
    final _FakeNpuDownloader downloader = _FakeNpuDownloader(bridge)
      ..script = <NpuDownloadProgress>[
        const NpuDownloadProgress(state: 'failed', error: 'corrupted'),
      ];
    final TranscriptionSettingsController controller =
        await _pump(tester, bridge, npuDownloader: downloader);
    await controller.downloadNpu();
    await tester.pump(const Duration(milliseconds: 10));
    await tester.pumpAndSettle();
    expect(controller.message, contains('failed its check'));
    expect(bridge.calls, isNot(contains('npu true')));
  });

  testWidgets('the NPU can be turned off and its files removed',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..npu = const NpuStatus(
          available: true,
          supported: true,
          directory: '/data/npu',
          build: _eightGen3,
          installed: true,
          enabled: true);
    final _FakeNpuDownloader downloader = _FakeNpuDownloader(bridge);
    await _pump(tester, bridge, npuDownloader: downloader);
    await tester.scrollUntilVisible(find.byKey(npuSwitchKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(npuSwitchKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, contains('npu false'));

    await tester.scrollUntilVisible(find.byKey(npuRemoveKey), 200,
        scrollable: find.byType(Scrollable).first);
    await tester.tap(find.byKey(npuRemoveKey));
    await tester.pumpAndSettle();
    expect(downloader.calls, contains('remove /data/npu'));
    await tester.scrollUntilVisible(find.byKey(npuDownloadKey), -200,
        scrollable: find.byType(Scrollable).first);
    expect(find.byKey(npuDownloadKey), findsOneWidget);
  });

  for (final (String lastOpen, String expected) in <(String, String)>[
    ('loaded', 'Used at the last recording.'),
    ('failed', "This phone's NPU failed its check"),
  ]) {
    testWidgets('the NPU switch says the NPU $lastOpen at the last recording',
        (WidgetTester tester) async {
      final _FakeBridge bridge = _FakeBridge()
        ..npu = NpuStatus(
            available: true,
            supported: true,
            directory: '/data/npu',
            build: _eightGen3,
            installed: true,
            enabled: true,
            lastOpen: lastOpen);
      await _pump(tester, bridge);
      await tester.scrollUntilVisible(find.byKey(npuSwitchKey), 200,
          scrollable: find.byType(Scrollable).first);
      await tester.pumpAndSettle();
      expect(find.textContaining(expected), findsOneWidget);
    });
  }
}
