// Runs a meeting export from the app (Issue #48) and words its result for a snack bar. Desktops
// ask where to save and let the core write there (or, without a save dialog, to the default
// export folder). Phones let the core write a private copy, pass its bytes to the system save
// dialog, and delete the copy, so the export never lingers in app storage. On Android an export
// can instead be shared with another app (#114): the core writes it, with its attachments, into
// the share folder, which is emptied before each share and at start-up.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Owns export state for the meeting screens.
final class MeetingExportController extends ChangeNotifier {
  /// Creates a controller over the core bridge and the save dialogs.
  MeetingExportController({
    required ExportBridge exportBridge,
    required ExportSaver exportSaver,
    ExportSharer? exportSharer,
  })  : _exportBridge = exportBridge,
        _exportSaver = exportSaver,
        _exportSharer = exportSharer;

  final ExportBridge _exportBridge;
  final ExportSaver _exportSaver;

  /// The share sheet, or null where exports cannot be shared (desktops).
  final ExportSharer? _exportSharer;
  bool _isExporting = false;

  /// Whether exports can be shared with another app.
  bool get canShare => _exportSharer != null;

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

  /// Exports one meeting and shares it with an app the owner picks.
  ///
  /// Inputs: The meeting, its displayed title (the file name and the share's text), the format.
  /// Outputs: null once the share sheet opened (or while another export runs, or without a
  /// sharer), or the message to show.
  /// Side effects: Empties the share folder, then the core writes the export there with its
  /// attachments; a Markdown export's attachments folder is flattened into the share folder,
  /// since only files directly in it can be served. Opens the share sheet. Notifies listeners.
  Future<String?> shareMeeting({
    required String meetingId,
    required String title,
    required MeetingExportFormat format,
  }) async {
    final ExportSharer? sharer = _exportSharer;
    if (_isExporting || sharer == null) {
      return null;
    }
    _isExporting = true;
    notifyListeners();
    try {
      final Directory folder = Directory(await sharer.shareDirectory());
      await emptyShareFolder(folder);
      final MeetingExportResult result = await _exportBridge.exportMeeting(
          meetingId, format,
          outputPath:
              '${folder.path}/${suggestedExportFileName(title, format)}');
      switch (result) {
        case final MeetingExportFailure failure:
          return _failureMessage(failure);
        case MeetingExportSuccess(:final String filePath):
          final List<String> paths = <String>[
            filePath,
            ...await _flattenAttachments(folder, filePath),
          ];
          await sharer.shareFiles(paths, title);
          return null;
      }
    } on Object {
      return l10n.unableToShareTheExport;
    } finally {
      _isExporting = false;
      notifyListeners();
    }
  }

  /// Moves a Markdown export's `<stem>_attachments/` files up into `folder`, renaming clashes
  /// as `name (2).ext`, and removes the emptied subfolder.
  ///
  /// Outputs: The moved files' paths, by name. Side effects: Renames files and deletes a folder.
  Future<List<String>> _flattenAttachments(
      Directory folder, String exportPath) async {
    final int extensionStart = exportPath.lastIndexOf('.');
    final Directory attachments = Directory(
        '${extensionStart > 0 ? exportPath.substring(0, extensionStart) : exportPath}_attachments');
    if (!await attachments.exists()) {
      return <String>[];
    }
    final List<File> files = <File>[
      await for (final FileSystemEntity entry in attachments.list())
        if (entry is File) entry,
    ]..sort((File a, File b) => a.path.compareTo(b.path));
    final List<String> moved = <String>[];
    for (final File file in files) {
      final String target = _freeName(folder, file.uri.pathSegments.last);
      moved.add((await file.rename(target)).path);
    }
    await attachments.delete(recursive: true);
    return moved;
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
      MeetingExportSuccess(:final String filePath) => l10n.exportedTo(filePath),
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
          return saved ? l10n.exportSaved : null;
        } on Object {
          return l10n.unableToSaveTheExport;
        } finally {
          if (await exportedFile.exists()) {
            await exportedFile.delete();
          }
        }
    }
  }
}

/// A path in `folder` for `name` that no file uses yet: `name`, else `stem (2).ext`, and so on.
String _freeName(Directory folder, String name) {
  final int dot = name.lastIndexOf('.');
  final String stem = dot > 0 ? name.substring(0, dot) : name;
  final String extension = dot > 0 ? name.substring(dot) : '';
  String candidate = name;
  for (int copy = 2; File('${folder.path}/$candidate').existsSync(); copy++) {
    candidate = '$stem ($copy)$extension';
  }
  return '${folder.path}/$candidate';
}

/// Deletes what an earlier share or hand-off left in `folder`, creating it when missing.
///
/// Side effects: Deletes files and subfolders in it (plain copies of meeting content).
Future<void> emptyShareFolder(Directory folder) async {
  if (!await folder.exists()) {
    await folder.create(recursive: true);
    return;
  }
  await for (final FileSystemEntity entry in folder.list()) {
    await entry.delete(recursive: true);
  }
}

/// Words a failure without exposing paths or internal messages.
String _failureMessage(MeetingExportFailure failure) =>
    switch (failure.errorCode) {
      'meeting_not_found' => l10n.thisMeetingNoLongerExists,
      'runtime_unavailable' => l10n.exportIsUnavailableUntilTheApp,
      _ => l10n.unableToExportTheMeeting,
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
