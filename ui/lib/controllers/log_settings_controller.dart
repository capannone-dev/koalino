// Holds the Settings screen's diagnostics (Issue #46): the core's log level and exporting the
// logs. Desktops ask where to save and let the core write there. Android has the core write into
// the app's `cache/shared` folder, the only folder its share provider serves, and opens the share
// sheet; the previous shared export is removed first, so at most one copy lingers.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/log_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Owns the log level and log export state for Settings.
final class LogSettingsController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The core bridge; the desktop save dialog; on Android, the share sheet (its
  /// presence selects the share flow); the folder Android shares from; and a clock for the
  /// file name (tests fix it).
  /// Side effects: None until `load`.
  LogSettingsController({
    required LogBridge logBridge,
    required ExportSaver exportSaver,
    required Future<Directory> Function() sharedDirectory,
    LogSharer? logSharer,
    DateTime Function() now = DateTime.now,
  })  : _logBridge = logBridge,
        _exportSaver = exportSaver,
        _sharedDirectory = sharedDirectory,
        _logSharer = logSharer,
        _now = now;

  final LogBridge _logBridge;
  final ExportSaver _exportSaver;
  final Future<Directory> Function() _sharedDirectory;
  final LogSharer? _logSharer;
  final DateTime Function() _now;

  LogLevel? _level;
  String? _levelMessage;
  bool _isExporting = false;

  /// The level in effect, or `null` before `load`, when the core is unavailable, or for a
  /// directive Settings does not offer.
  LogLevel? get level => _level;

  /// Why the last level change failed, or `null`.
  String? get levelMessage => _levelMessage;

  /// Whether an export is running; the action is disabled meanwhile.
  bool get isExporting => _isExporting;

  /// Whether exporting opens the share sheet (Android) instead of a save dialog.
  bool get shares => _logSharer != null;

  /// Reads the stored level from the core.
  ///
  /// Side effects: Notifies listeners.
  Future<void> load() async {
    final String? directive = await _logBridge.currentLevel();
    _level = directive == null ? null : LogLevel.fromDirective(directive);
    notifyListeners();
  }

  /// Changes the core's level.
  ///
  /// Inputs: The new level. Outputs: None; on failure the old level stays and `levelMessage`
  /// explains. Side effects: The core stores and applies it; notifies listeners.
  Future<void> setLevel(LogLevel level) async {
    final bool accepted = await _logBridge.setLevel(level.directive);
    if (accepted) {
      _level = level;
      _levelMessage = null;
    } else {
      _levelMessage = l10n.unableToChangeTheLogLevel;
    }
    notifyListeners();
  }

  /// Exports the newest week of logs.
  ///
  /// Outputs: The message to show, or `null` when the owner cancelled the save dialog or the
  /// share sheet opened (it is its own feedback). Ignored (returns `null`) while another export
  /// runs.
  /// Side effects: Shows a save dialog or the share sheet; the core writes the export. On Android
  /// earlier files in the shared folder are deleted. Notifies listeners at the start and end.
  Future<String?> exportLogs() async {
    if (_isExporting) {
      return null;
    }
    _isExporting = true;
    notifyListeners();
    try {
      final String fileName = _exportFileName(_now());
      final LogSharer? logSharer = _logSharer;
      return logSharer == null
          ? await _exportToChosenPath(fileName)
          : await _exportAndShare(logSharer, fileName);
    } finally {
      _isExporting = false;
      notifyListeners();
    }
  }

  /// Desktop flow: the core writes where the owner chose.
  Future<String?> _exportToChosenPath(String fileName) async {
    String? outputPath;
    try {
      outputPath = await _exportSaver.chooseSavePath(fileName);
      if (outputPath == null) {
        return null;
      }
    } on Object {
      // No save dialog on this system: the core writes to its export folder.
      outputPath = null;
    }
    return switch (await _logBridge.exportLogs(outputPath: outputPath)) {
      LogExportSuccess(:final String filePath) => l10n.logsExportedTo(filePath),
      final LogExportFailure failure => _failureMessage(failure),
    };
  }

  /// Android flow: the core writes into the shared folder, then the share sheet opens.
  Future<String?> _exportAndShare(LogSharer logSharer, String fileName) async {
    final Directory sharedDirectory = await _sharedDirectory();
    if (sharedDirectory.existsSync()) {
      await sharedDirectory.delete(recursive: true);
    }
    await sharedDirectory.create(recursive: true);
    switch (await _logBridge.exportLogs(
        outputPath: '${sharedDirectory.path}/$fileName')) {
      case final LogExportFailure failure:
        return _failureMessage(failure);
      case LogExportSuccess(:final String filePath):
        try {
          await logSharer.share(filePath);
          return null;
        } on Object {
          return l10n.unableToShareTheLogs;
        }
    }
  }
}

/// `note_taker_logs_<YYYYMMDD-HHMMSS>.jsonl`, matching the core's own export name.
String _exportFileName(DateTime time) {
  String twoDigits(int value) => value.toString().padLeft(2, '0');
  return 'note_taker_logs_${time.year}${twoDigits(time.month)}'
      '${twoDigits(time.day)}-${twoDigits(time.hour)}${twoDigits(time.minute)}'
      '${twoDigits(time.second)}.jsonl';
}

/// Words a failure without exposing paths or internal messages.
String _failureMessage(LogExportFailure failure) => switch (failure.errorCode) {
      'no_logs' => l10n.noLogsToExportYet,
      _ => l10n.unableToExportTheLogs,
    };
