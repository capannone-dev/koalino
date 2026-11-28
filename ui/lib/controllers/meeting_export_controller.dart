// Runs a meeting export from the app (Issue #48) and words its result for a snack bar. Desktops
// ask where to save and let the core write there (or, without a save dialog, to the default
// export folder). Phones let the core write a private copy, pass its bytes to the system save
// dialog, and delete the copy, so the export never lingers in app storage. On Android an export
// can instead be shared with another app (#114): the core writes it, with its attachments, into
// the share folder, which is emptied before each share and at start-up. A kept recording (#199)
// is shared the same way on Android, or saved where the owner chooses on desktops.

import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Owns export state for the meeting screens.
final class MeetingExportController extends ChangeNotifier {
  /// Creates a controller over the core bridge and the save dialogs.
  ///
  /// `recordingSaver` is the dialog a kept recording is saved with (#199; `exportSaver` when not
  /// given), and `downloadsDirectory` finds where it goes when no dialog can open (the system's
  /// Downloads folder by default).
  MeetingExportController({
    required ExportBridge exportBridge,
    required ExportSaver exportSaver,
    ExportSharer? exportSharer,
    ExportSaver? recordingSaver,
    Future<String?> Function() downloadsDirectory = downloadsDirectoryPath,
  })  : _exportBridge = exportBridge,
        _exportSaver = exportSaver,
        _exportSharer = exportSharer,
        _recordingSaver = recordingSaver ?? exportSaver,
        _downloadsDirectory = downloadsDirectory;

  final ExportBridge _exportBridge;
  final ExportSaver _exportSaver;

  /// The share sheet, or null where exports cannot be shared (desktops).
  final ExportSharer? _exportSharer;
  final ExportSaver _recordingSaver;
  final Future<String?> Function() _downloadsDirectory;
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

  /// Exports several meetings at once (#216): on Android they are shared in one share sheet; on
  /// desktops the core writes each to its export folder, without a save dialog per meeting.
  ///
  /// Inputs: The meetings as (ID, displayed title) and the format.
  /// Outputs: The message to show, or null once the share sheet opened or while another export
  /// runs.
  /// Side effects: Android: empties the share folder, writes the exports there and opens the share
  /// sheet. Desktops: the core writes the files. Notifies listeners.
  Future<String?> exportMeetings(
      List<({String meetingId, String title})> meetings,
      MeetingExportFormat format) async {
    if (_isExporting || meetings.isEmpty) {
      return null;
    }
    _isExporting = true;
    notifyListeners();
    try {
      final ExportSharer? sharer = _exportSharer;
      final Directory? shareFolder =
          sharer == null ? null : Directory(await sharer.shareDirectory());
      if (shareFolder != null) {
        await emptyShareFolder(shareFolder);
      }
      final List<String> written = <String>[];
      int failed = 0;
      for (final (:String meetingId, :String title) in meetings) {
        final String name = suggestedExportFileName(title, format);
        final MeetingExportResult result = await _exportBridge.exportMeeting(
            meetingId, format,
            outputPath:
                shareFolder == null ? null : _freeName(shareFolder, name),
            includeAttachments:
                shareFolder == null || format != MeetingExportFormat.markdown);
        switch (result) {
          case MeetingExportSuccess(:final String filePath):
            written.add(filePath);
          case MeetingExportFailure():
            failed++;
        }
      }
      if (sharer != null && written.isNotEmpty) {
        await sharer.shareFiles(written, l10n.meetingsCount(written.length));
        return failed == 0 ? null : l10n.someMeetingsCouldNotBeExported;
      }
      if (written.isEmpty) {
        return l10n.someMeetingsCouldNotBeExported;
      }
      final String folder = File(written.first).parent.path;
      return failed == 0
          ? l10n.exportedMeetingsTo(written.length, folder)
          : '${l10n.exportedMeetingsTo(written.length, folder)} '
              '${l10n.someMeetingsCouldNotBeExported}';
    } on Object {
      return l10n.someMeetingsCouldNotBeExported;
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

  /// Shares the meeting's kept recording as a WAV on Android, or saves it where the owner
  /// chooses elsewhere (#199).
  ///
  /// Inputs: The meeting, its displayed title (the file name and the share's text), and the
  /// recordings bridge. Outputs: The message to show, or `null` when the share sheet opened, the
  /// owner cancelled the dialog, or another export runs.
  /// Side effects: Android: empties the share folder, the core writes the decrypted recording
  /// there (removed at the next share or start-up), and the share sheet opens. Desktops: shows a
  /// save dialog and the core writes the recording at the chosen path (or, with no dialog, as a
  /// new file in the Downloads folder). Notifies listeners.
  Future<String?> exportRecording({
    required String meetingId,
    required String title,
    required RecordingsBridge recordings,
  }) async {
    if (_isExporting) {
      return null;
    }
    final ExportSharer? sharer = _exportSharer;
    final String fileName = suggestedRecordingFileName(title);
    _isExporting = true;
    notifyListeners();
    try {
      if (sharer != null) {
        final Directory folder = Directory(await sharer.shareDirectory());
        await emptyShareFolder(folder);
        final String path =
            await recordings.export(meetingId, '${folder.path}/$fileName');
        await sharer.shareFiles(<String>[path], title);
        return null;
      }
      String? chosenPath;
      try {
        chosenPath = await _recordingSaver.chooseSavePath(fileName);
        if (chosenPath == null) {
          return null;
        }
      } on Object {
        // No save dialog on this system: the Downloads folder, never replacing a file there.
        final String? downloads = await _downloadsDirectory();
        if (downloads == null) {
          return l10n.unableToSaveTheRecording;
        }
        chosenPath = _freeName(Directory(downloads), fileName);
      }
      return l10n.exportedTo(await recordings.export(meetingId, chosenPath));
    } on Object {
      return sharer != null
          ? l10n.unableToShareTheRecording
          : l10n.unableToSaveTheRecording;
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
String suggestedExportFileName(String title, MeetingExportFormat format) =>
    _fileNameFor(title, format.fileExtension);

/// Suggests a kept recording's file name from the meeting title: `<title>.wav`, cleaned as
/// [suggestedExportFileName] does (#199).
String suggestedRecordingFileName(String title) => _fileNameFor(title, 'wav');

/// `<title>.<extension>` with characters file systems reject replaced by `_`.
String _fileNameFor(String title, String extension) {
  final String safeTitle =
      title.replaceAll(RegExp(r'[\\/:*?"<>|\x00-\x1F]'), '_').trim();
  return '${safeTitle.isEmpty ? 'meeting' : safeTitle}.$extension';
}
