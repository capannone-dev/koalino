// Scripted log bridge, sharer, and save dialog for the Settings tests (Issue #46). The bridge
// writes a small file where the core would, so the controller's file handling runs for real.

import 'dart:io';
import 'dart:typed_data';

import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/log_bridge.dart';

/// Holds a log level and exports a small file, or fails as scripted.
final class FakeLogBridge implements LogBridge {
  FakeLogBridge({
    this.storedLevel = 'info',
    this.acceptsLevels = true,
    this.exportFailure,
  });

  /// The level the core reports (`null`: the core is unavailable).
  String? storedLevel;

  /// Whether `setLevel` succeeds.
  final bool acceptsLevels;

  /// When set, every export fails with it.
  final LogExportFailure? exportFailure;

  final List<String> requestedLevels = <String>[];
  final List<String?> exportPaths = <String?>[];

  @override
  Future<String?> currentLevel() async => storedLevel;

  @override
  Future<bool> setLevel(String directive) async {
    requestedLevels.add(directive);
    if (acceptsLevels) {
      storedLevel = directive;
    }
    return acceptsLevels;
  }

  @override
  Future<LogExportResult> exportLogs({String? outputPath}) async {
    exportPaths.add(outputPath);
    final LogExportFailure? failure = exportFailure;
    if (failure != null) {
      return failure;
    }
    final String path = outputPath ?? '${Directory.systemTemp.path}/logs.jsonl';
    File(path).writeAsStringSync('{"level":"INFO"}\n');
    return LogExportSuccess(filePath: path, sizeBytes: 17);
  }
}

/// Records the files handed to the share sheet, or fails.
final class FakeLogSharer implements LogSharer {
  FakeLogSharer({this.fails = false});

  final bool fails;
  final List<String> sharedPaths = <String>[];

  @override
  Future<void> share(String filePath) async {
    if (fails) {
      throw const FileSystemException('no app can receive the file');
    }
    sharedPaths.add(filePath);
  }
}

/// A desktop save dialog that returns `chosenPath` (`null`: cancelled).
final class FakeDesktopSaver implements ExportSaver {
  FakeDesktopSaver({this.chosenPath});

  final String? chosenPath;
  final List<String> suggestedNames = <String>[];

  @override
  bool get savesBytes => false;

  @override
  Future<String?> chooseSavePath(String suggestedFileName) async {
    suggestedNames.add(suggestedFileName);
    return chosenPath;
  }

  @override
  Future<bool> saveBytes(String suggestedFileName, Uint8List bytes) =>
      throw UnsupportedError('desktops save by path');
}
