// Remembers which one-time tips the owner has dismissed (#213), one tip name per line in
// `<data directory>/ui-seen-tips`. Only the app reads it; a tip shows again if the file is lost.

import 'dart:io';

import 'package:note_taker_ui/infrastructure/app_configuration.dart';

/// Reads and records dismissed tips.
abstract interface class SeenTipsStore {
  /// Whether the tip named `tip` was dismissed; false when unknown or unreadable.
  Future<bool> seen(String tip);

  /// Records that `tip` was dismissed. Failures are swallowed: the tip then returns next run.
  Future<void> markSeen(String tip);
}

/// Keeps the dismissed tips in a file in the app's data directory.
final class FileSeenTipsStore implements SeenTipsStore {
  /// Creates the store; `directory` returns the folder to use (replaced in tests).
  const FileSeenTipsStore({this.directory = appDataDirectory});

  /// Returns the folder holding the file.
  final Future<Directory> Function() directory;

  Future<File> _file() async =>
      File('${(await directory()).path}${Platform.pathSeparator}ui-seen-tips');

  Future<Set<String>> _read() async {
    try {
      return (await (await _file()).readAsLines())
          .map((String line) => line.trim())
          .where((String line) => line.isNotEmpty)
          .toSet();
    } on Object {
      return <String>{};
    }
  }

  /// Side effects: Reads the file.
  @override
  Future<bool> seen(String tip) async => (await _read()).contains(tip);

  /// Side effects: Creates the data directory if needed and rewrites the file.
  @override
  Future<void> markSeen(String tip) async {
    try {
      final Set<String> tips = await _read()
        ..add(tip);
      final File file = await _file();
      await file.parent.create(recursive: true);
      await file.writeAsString('${tips.join('\n')}\n', flush: true);
    } on Object {
      return;
    }
  }
}

/// A store kept in memory, for tests and platforms without a data directory.
final class MemorySeenTipsStore implements SeenTipsStore {
  /// Creates an empty store, or one with `tips` already dismissed.
  MemorySeenTipsStore([Iterable<String> tips = const <String>[]])
      : _tips = tips.toSet();

  final Set<String> _tips;

  @override
  Future<bool> seen(String tip) async => _tips.contains(tip);

  @override
  Future<void> markSeen(String tip) async => _tips.add(tip);
}
