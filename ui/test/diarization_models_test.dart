// Specifies how the app finds the diarization models it ships: desktops pass the bundle's folder
// when both files are there, and Android installs both from the APK into the data directory,
// where the Rust core finds them by itself.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/diarization_models.dart';

void main() {
  late Directory root;

  setUp(
      () => root = Directory.systemTemp.createTempSync('diarization-models-'));
  tearDown(() => root.deleteSync(recursive: true));

  /// Creates `<bundle>/data/models/diarization/<fileName>`.
  void createBundled(String fileName) =>
      File('${root.path}/data/models/diarization/$fileName')
          .createSync(recursive: true);

  test('the bundled folder sits in data/models beside the executable', () {
    expect(bundledDiarizationModelDirectory('/opt/note_taker/note_taker_ui'),
        '/opt/note_taker/data/models/diarization');
    expect(
        bundledDiarizationModelDirectory(
            r'C:\Apps\Note Taker\note_taker_ui.exe'),
        r'C:\Apps\Note Taker\data\models\diarization');
  });

  test('a macOS app keeps the models in its Resources folder (#151)', () {
    expect(
        bundledDiarizationModelDirectory(
            '/Applications/Koalino.app/Contents/MacOS/Koalino'),
        '/Applications/Koalino.app/Contents/Resources/models/diarization');
  });

  test('desktops pass the bundled folder only when both models are there',
      () async {
    Future<String?> resolve() => resolveDiarizationModelDirectory(
          dataDirectory: '${root.path}/own',
          executablePath: '${root.path}/note_taker_ui',
          isAndroid: false,
          installAndroidModel: (_, __) async => fail('not Android'),
        );

    expect(await resolve(), isNull);
    createBundled('segmentation.onnx');
    expect(await resolve(), isNull);
    createBundled('embedding.onnx');
    expect(await resolve(), '${root.path}/data/models/diarization');
  });

  test('Android installs both models into the data directory', () async {
    final Map<String, String> installed = <String, String>{};

    final String? resolved = await resolveDiarizationModelDirectory(
      dataDirectory: '/data/app',
      executablePath: '/system/bin/app_process',
      isAndroid: true,
      installAndroidModel: (String assetPath, String targetPath) async {
        installed[assetPath] = targetPath;
        return true;
      },
    );

    expect(resolved, isNull);
    expect(installed, <String, String>{
      'models/diarization/segmentation.onnx':
          '/data/app/models/diarization/segmentation.onnx',
      'models/diarization/embedding.onnx':
          '/data/app/models/diarization/embedding.onnx',
    });
  });
}
