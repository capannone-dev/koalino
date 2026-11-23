// Specifies how the app finds its Whisper model so a fresh install transcribes out of the box:
// a model the owner placed in the data directory wins; otherwise desktops use the copy shipped
// in the bundle and Android installs the one shipped in the APK.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/whisper_model.dart';

void main() {
  late Directory root;

  setUp(() => root = Directory.systemTemp.createTempSync('whisper-model-'));
  tearDown(() => root.deleteSync(recursive: true));

  /// Creates `relativePath` under the temporary root and returns its absolute path.
  String createFile(String relativePath) {
    final File file = File('${root.path}/$relativePath')
      ..createSync(recursive: true)
      ..writeAsStringSync('model');
    return file.path;
  }

  test('the bundled desktop model sits in data/models beside the executable',
      () {
    expect(bundledDesktopModelPath('/opt/note_taker/note_taker_ui'),
        '/opt/note_taker/data/models/ggml-base.bin');
    expect(bundledDesktopModelPath(r'C:\Apps\Note Taker\note_taker_ui.exe'),
        r'C:\Apps\Note Taker\data\models\ggml-base.bin');
  });

  test('a macOS app keeps the model in its Resources folder (#151)', () {
    expect(
        bundledDesktopModelPath(
            '/Applications/Koalino.app/Contents/MacOS/Koalino'),
        '/Applications/Koalino.app/Contents/Resources/models/ggml-base.bin');
  });

  test('a model in the data directory wins over the bundled one', () async {
    final String ownModel = createFile('data/models/ggml-base.bin');
    createFile('bundle/data/models/ggml-base.bin');

    final String? resolved = await resolveWhisperModelPath(
      dataDirectory: '${root.path}/data',
      executablePath: '${root.path}/bundle/note_taker_ui',
      isAndroid: false,
      installAndroidModel: (_) async => fail('not Android'),
    );

    expect(resolved, ownModel);
  });

  test('desktops fall back to the model shipped in the bundle', () async {
    final String bundled = createFile('bundle/data/models/ggml-base.bin');

    final String? resolved = await resolveWhisperModelPath(
      dataDirectory: '${root.path}/data',
      executablePath: '${root.path}/bundle/note_taker_ui',
      isAndroid: false,
      installAndroidModel: (_) async => fail('not Android'),
    );

    expect(resolved, bundled);
  });

  test('without any model the core is configured without one', () async {
    final String? resolved = await resolveWhisperModelPath(
      dataDirectory: '${root.path}/data',
      executablePath: '${root.path}/bundle/note_taker_ui',
      isAndroid: false,
      installAndroidModel: (_) async => fail('not Android'),
    );

    expect(resolved, isNull);
  });

  test('Android installs the APK model into the data directory', () async {
    final List<String> installTargets = <String>[];

    final String? resolved = await resolveWhisperModelPath(
      dataDirectory: '${root.path}/data',
      executablePath: '/system/bin/app_process64',
      isAndroid: true,
      installAndroidModel: (String target) async {
        installTargets.add(target);
        return true;
      },
    );

    final String expected = '${root.path}/data/models/ggml-base.bin';
    expect(installTargets, <String>[expected]);
    expect(resolved, expected);
  });

  test('Android without a shipped model is configured without one', () async {
    final String? resolved = await resolveWhisperModelPath(
      dataDirectory: '${root.path}/data',
      executablePath: '/system/bin/app_process64',
      isAndroid: true,
      installAndroidModel: (_) async => false,
    );

    expect(resolved, isNull);
  });
}
