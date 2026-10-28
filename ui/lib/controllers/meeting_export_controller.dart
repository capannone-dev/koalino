// Runs a meeting export from the app (Issue #48) and words its result for a snack bar. Desktops
// ask where to save and let the core write there (or, without a save dialog, to the default
// export folder). Phones let the core write a private copy, pass its bytes to the system save
// dialog, and delete the copy, so the export never lingers in app storage.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';

/// Owns export state for the meeting screens.
final class MeetingExportController extends ChangeNotifier {
  /// Creates a controller over the core bridge and the save dialogs.
  MeetingExportController({
    required ExportBridge exportBridge,
    required ExportSaver exportSaver,
  })  : _exportBridge = exportBridge,
        _exportSaver = exportSaver;

  final ExportBridge _exportBridge;
  final ExportSaver _exportSaver;
  bool _isExporting = false;

  /// Whether an export is running; the Export action is disabled meanwhile.
  bool get isExporting => _isExporting;

  /// Exports one meeting and saves it where the owner chooses.
  ///
  /// Inputs: The meeting, its displayed title (for the suggested file name), and the format.
  /// Outputs: The message to show, or `null` when the owner cancelled the save dialog. Ignored
  /// (returns `null`) while another export runs.
  /// Side effects: Shows a save dialog; the core writes the file. On phones the private copy
  /// is deleted afterwards, and Markdown is exported without attachments, because the dialog
  /// saves a single file. Notifies listeners when the export starts and ends.
  Future<String?> exportMeeting({
    required String meetingId,
    required String title,
    required MeetingExportFormat format,
  }) async {
    if (_isExporting) {
      return null;
    }
    _isExporting = true;
    notifyListeners();
    try {
      final String suggestedName = suggestedExportFileName(title, format);
      return _exportSaver.savesBytes
          ? await _exportThroughBytes(meetingId, format, suggestedName)
          : await _exportToChosenPath(meetingId, format, suggestedName);
    } finally {
      _isExporting = false;
      notifyListeners();
    }
  }

  /// Desktop flow: the core writes where the owner chose.
  Future<String?> _exportToChosenPath(String meetingId,
      MeetingExportFormat format, String suggestedName) async {
    String? outputPath;
    try {
      outputPath = await _exportSaver.chooseSavePath(suggestedName);
      if (outputPath == null) {
        return null;
      }
    } on Object {
      // No save dialog on this system: fall back to the default export folder.
      outputPath = null;
    }
    final MeetingExportResult result = await _exportBridge
        .exportMeeting(meetingId, format, outputPath: outputPath);
    return switch (result) {
      MeetingExportSuccess(:final String filePath) => 'Exported to $filePath',
      final MeetingExportFailure failure => _failureMessage(failure),
    };
  }

  /// Phone flow: a private copy goes to the system save dialog, then is deleted.
  Future<String?> _exportThroughBytes(String meetingId,
      MeetingExportFormat format, String suggestedName) async {
    final MeetingExportResult result = await _exportBridge.exportMeeting(
        meetingId, format,
        includeAttachments: format != MeetingExportFormat.markdown);
    switch (result) {
      case final MeetingExportFailure failure:
        return _failureMessage(failure);
      case MeetingExportSuccess(:final String filePath):
        final File exportedFile = File(filePath);
        try {
          final Uint8List bytes = await exportedFile.readAsBytes();
          final bool saved = await _exportSaver.saveBytes(suggestedName, bytes);
          return saved ? 'Export saved' : null;
        } on Object {
          return 'Unable to save the export';
        } finally {
          if (await exportedFile.exists()) {
            await exportedFile.delete();
          }
        }
    }
  }
}

/// Words a failure without exposing paths or internal messages.
String _failureMessage(MeetingExportFailure failure) =>
    switch (failure.errorCode) {
      'meeting_not_found' => 'This meeting no longer exists',
      'runtime_unavailable' =>
        'Export is unavailable until the app has started',
      _ => 'Unable to export the meeting',
    };

/// Suggests the export's file name from the meeting title.
///
/// Inputs: The displayed title and the format.
/// Outputs: `<title>.<extension>` with characters that Windows, Android, or Linux reject in file
/// names replaced by `_`; `meeting.<extension>` for a blank title.
/// Side effects: None.
String suggestedExportFileName(String title, MeetingExportFormat format) {
  final String safeTitle =
      title.replaceAll(RegExp(r'[\\/:*?"<>|\x00-\x1F]'), '_').trim();
  return '${safeTitle.isEmpty ? 'meeting' : safeTitle}.${format.fileExtension}';
}
