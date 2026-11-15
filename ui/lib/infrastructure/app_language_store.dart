// Keeps the app language the owner picked (Issue #124) in a one-line file in the app's data
// directory, `ui-language`. Flutter owns this setting: the Rust core never reads the app's text.

import 'dart:io';

import 'package:note_taker_ui/infrastructure/app_configuration.dart';

/// Reads and writes the app language choice.
abstract interface class AppLanguageStore {
  /// Returns the stored language code, or `null` when none is stored or it cannot be read.
  Future<String?> read();

  /// Stores `code`, or removes the choice with `null`. Failures are swallowed: the choice then
  /// lasts for this run only.
  Future<void> write(String? code);
}

/// Stores the choice in `<data directory>/ui-language`.
final class FileAppLanguageStore implements AppLanguageStore {
  /// Creates the store; `directory` returns the folder to use (the app's data directory unless
  /// replaced in tests).
  const FileAppLanguageStore({this.directory = appDataDirectory});

  /// Returns the folder holding the file.
  final Future<Directory> Function() directory;

  Future<File> _file() async =>
      File('${(await directory()).path}${Platform.pathSeparator}ui-language');

  /// Side effects: Reads the file.
  @override
  Future<String?> read() async {
    try {
      final String text = (await (await _file()).readAsString()).trim();
      return text.isEmpty ? null : text;
    } on Object {
      return null;
    }
  }

  /// Side effects: Creates the data directory if needed, then writes or deletes the file.
  @override
  Future<void> write(String? code) async {
    try {
      final File file = await _file();
      if (code == null) {
        if (await file.exists()) {
          await file.delete();
        }
        return;
      }
      await file.parent.create(recursive: true);
      await file.writeAsString('$code\n', flush: true);
    } on Object {
      return;
    }
  }
}
