// Specifies the first-start download of Automatic's model (#189): the app transcribes with the
// shipped model at once and fetches the better one by itself, only on an unmetered network, only
// when it fits, never twice in a run after a failure or a stop.
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/l10n/app_localizations.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';
import 'package:note_taker_ui/controllers/automatic_model_fetcher.dart';
import 'package:note_taker_ui/infrastructure/network_state.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';

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

/// Serves settings like the core and records downloads; a download runs until `finish`.
final class _FakeBridge extends Fake implements TranscriptionSettingsBridge {
  bool autoDownload = true;
  bool automatic = true;
  String? waiting = 'ggml-small.bin';
  int? freeBytes = 10000000000;
  WhisperDownload current = const WhisperDownload();
  final List<String> calls = <String>[];

  @override
  Future<TranscriptionSettings?> read() async => TranscriptionSettings(
        selectedModel: automatic ? null : 'ggml-base.bin',
        availableDiskBytes: freeBytes,
        models: const <WhisperModelInfo>[_base, _small],
        download: current,
        autoDownload: autoDownload,
        automatic: AutomaticModelStatus(
          enabled: automatic,
          fileName: 'ggml-base.bin',
          waitingForDownload: automatic ? waiting : null,
        ),
      );

  @override
  Future<int> download(String fileName) async {
    calls.add('download $fileName');
    current = WhisperDownload(
        fileName: fileName,
        receivedBytes: 0,
        totalBytes: 487601967,
        state: 'downloading');
    return transcriptionStatusOk;
  }

  @override
  Future<void> cancelDownload() async {
    calls.add('cancel');
    current = const WhisperDownload(state: 'cancelled');
  }

  void progress(int received) => current = WhisperDownload(
      fileName: 'ggml-small.bin',
      receivedBytes: received,
      totalBytes: 487601967,
      state: 'downloading');

  void finish(String state) {
    current = WhisperDownload(fileName: 'ggml-small.bin', state: state);
    if (state == 'done') {
      waiting = null;
    }
  }
}

final class _FakeNetwork implements NetworkState {
  bool unmetered = true;

  @override
  Future<bool> isUnmetered() async => unmetered;
}

AutomaticModelFetcher _fetcher(_FakeBridge bridge, _FakeNetwork network) =>
    AutomaticModelFetcher(
      bridge: bridge,
      network: network,
      pollInterval: Duration.zero,
    );

void main() {
  test('at start it downloads Automatic\'s model on Wi-Fi and reports progress',
      () async {
    final _FakeBridge bridge = _FakeBridge();
    final AutomaticModelFetcher fetcher = _fetcher(bridge, _FakeNetwork());
    await fetcher.check();
    expect(bridge.calls, <String>['download ggml-small.bin']);
    expect(fetcher.modelName, 'small');

    bridge.progress(243800983);
    await fetcher.poll();
    expect(fetcher.progress, closeTo(0.5, 0.01));

    bridge.finish('done');
    await fetcher.poll();
    expect(fetcher.modelName, isNull, reason: 'done: the banner goes away');
    fetcher.dispose();
  });

  test('on mobile data it waits for Wi-Fi, and starts at the next check',
      () async {
    final _FakeBridge bridge = _FakeBridge();
    final _FakeNetwork network = _FakeNetwork()..unmetered = false;
    final AutomaticModelFetcher fetcher = _fetcher(bridge, network);
    await fetcher.check();
    expect(bridge.calls, isEmpty);
    expect(fetcher.modelName, isNull);

    network.unmetered = true;
    await fetcher.check();
    expect(bridge.calls, <String>['download ggml-small.bin']);
    fetcher.dispose();
  });

  test('nothing downloads when turned off, hand-picked, too large or running',
      () async {
    final _FakeNetwork network = _FakeNetwork();
    for (final _FakeBridge bridge in <_FakeBridge>[
      _FakeBridge()..autoDownload = false,
      _FakeBridge()..automatic = false,
      _FakeBridge()..freeBytes = 150000000,
      _FakeBridge()..waiting = null,
      _FakeBridge()
        ..current = const WhisperDownload(
            fileName: 'ggml-medium.bin', state: 'downloading'),
    ]) {
      final AutomaticModelFetcher fetcher = _fetcher(bridge, network);
      await fetcher.check();
      expect(bridge.calls, isEmpty);
      fetcher.dispose();
    }
  });

  test('a stop or a failure is not retried until the next start', () async {
    final _FakeBridge bridge = _FakeBridge();
    final AutomaticModelFetcher fetcher = _fetcher(bridge, _FakeNetwork());
    await fetcher.check();
    await fetcher.stop();
    expect(bridge.calls, <String>['download ggml-small.bin', 'cancel']);
    expect(fetcher.modelName, isNull);
    await fetcher.check();
    expect(bridge.calls.length, 2, reason: 'stopped for this run');
    fetcher.dispose();

    final _FakeBridge failing = _FakeBridge();
    final AutomaticModelFetcher retrying = _fetcher(failing, _FakeNetwork());
    await retrying.check();
    failing.finish('failed');
    await retrying.poll();
    await retrying.check();
    expect(failing.calls, <String>['download ggml-small.bin']);
    retrying.dispose();
  });

  testWidgets('the Record screen banner shows progress and stops the download',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    final AutomaticModelFetcher fetcher = _fetcher(bridge, _FakeNetwork());
    await tester.pumpWidget(MaterialApp(
      localizationsDelegates: AppLocalizations.localizationsDelegates,
      supportedLocales: AppLocalizations.supportedLocales,
      home: Scaffold(body: ModelDownloadBanner(fetcher: fetcher)),
    ));
    expect(find.byKey(modelDownloadBannerKey), findsNothing);

    await tester.runAsync(fetcher.check);
    bridge.progress(243800984);
    await tester.runAsync(fetcher.poll);
    await tester.pump();
    expect(find.textContaining('small'), findsOneWidget);
    expect(find.textContaining('50%'), findsOneWidget);
    expect(
        find.text("Recordings keep using the current model until it's ready."),
        findsOneWidget);

    await tester.tap(find.text('Stop'));
    await tester.runAsync(() => Future<void>.delayed(Duration.zero));
    await tester.pump();
    expect(bridge.calls.last, 'cancel');
    expect(find.byKey(modelDownloadBannerKey), findsNothing);
    fetcher.dispose();
  });
}
