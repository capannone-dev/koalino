// Exports a meeting's summary as a PDF, plain text, Word or Markdown file (#130) and words the
// result for a snack bar. Like a meeting export, desktops ask where to save and let the core write
// there; phones let the core write a private copy, pass its bytes to the system save dialog, and
// delete the copy, so the summary never lingers in app storage.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Runs summary exports, one at a time.
final class SummaryExportController extends ChangeNotifier {
  /// Creates the controller.
  ///
  /// Inputs: The core bridge; the save dialogs; the app's private folder, where a phone's copy
  /// is written before it goes to the save dialog.
  SummaryExportController({
    required SummaryExportBridge bridge,
    required ExportSaver saver,
    required Future<String> Function() privateDirectory,
  })  : _bridge = bridge,
        _saver = saver,
        _privateDirectory = privateDirectory;

  final SummaryExportBridge _bridge;
  final ExportSaver _saver;
  final Future<String> Function() _privateDirectory;
  bool _isExporting = false;

  /// Whether an export runs; the Export menu is disabled meanwhile.
  bool get isExporting => _isExporting;

  /// Exports one meeting's summary and saves it where the owner chooses.
  ///
  /// Inputs: The meeting, its displayed title (for the suggested file name), and the format.
  /// Outputs: The message to show, or `null` when the owner cancelled (or another export runs).
  /// Side effects: Shows a save dialog; the core writes the file. On phones the private copy is
  /// deleted afterwards. Notifies listeners when the export starts and ends.
  Future<String?> exportSummary({
    required String meetingId,
    required String title,
    required SummaryExportFormat format,
  }) async {
    if (_isExporting) {
      return null;
    }
    _isExporting = true;
    notifyListeners();
    try {
      final String suggestedName = suggestedSummaryFileName(title, format);
      return _saver.savesBytes
          ? await _exportThroughBytes(meetingId, format, suggestedName)
          : await _exportToChosenPath(meetingId, format, suggestedName);
    } finally {
      _isExporting = false;
      notifyListeners();
    }
  }

  /// Desktop flow: the core writes where the owner chose.
  Future<String?> _exportToChosenPath(String meetingId,
      SummaryExportFormat format, String suggestedName) async {
    final String? outputPath;
    try {
      outputPath = await _saver.chooseSavePath(suggestedName);
    } on Object {
      return l10n.unableToSaveTheExport;
    }
    if (outputPath == null) {
      return null;
    }
    return switch (await _bridge.exportSummary(meetingId, format, outputPath)) {
      MeetingExportSuccess(:final String filePath) => l10n.exportedTo(filePath),
      final MeetingExportFailure failure => _failureMessage(failure),
    };
  }

  /// Phone flow: a private copy goes to the system save dialog, then is deleted.
  Future<String?> _exportThroughBytes(String meetingId,
      SummaryExportFormat format, String suggestedName) async {
    final File copy = File('${await _privateDirectory()}/$suggestedName');
    try {
      switch (await _bridge.exportSummary(meetingId, format, copy.path)) {
        case final MeetingExportFailure failure:
          return _failureMessage(failure);
        case MeetingExportSuccess():
          final Uint8List bytes = await copy.readAsBytes();
          return await _saver.saveBytes(suggestedName, bytes)
              ? l10n.exportSaved
              : null;
      }
    } on Object {
      return l10n.unableToSaveTheExport;
    } finally {
      if (await copy.exists()) {
        await copy.delete();
      }
    }
  }
}

/// Words a failed export.
String _failureMessage(MeetingExportFailure failure) =>
    switch (failure.errorCode) {
      'no_summary' => l10n.noSummaryYet,
      'meeting_not_found' => l10n.thisMeetingNoLongerExists,
      'runtime_unavailable' => l10n.exportIsUnavailableUntilTheApp,
      _ => l10n.unableToExportTheSummary,
    };

/// Suggests the file name: `<title> - <Summary>.<extension>`, with characters that Windows,
/// Android or Linux reject in file names replaced by `_`; `meeting` for a blank title.
///
/// Side effects: None.
String suggestedSummaryFileName(String title, SummaryExportFormat format) {
  final String safeTitle =
      title.replaceAll(RegExp(r'[\\/:*?"<>|\x00-\x1F]'), '_').trim();
  return '${safeTitle.isEmpty ? 'meeting' : safeTitle} - ${l10n.summary}'
      '.${format.fileExtension}';
}
