// Moves desktop data from the preview's folders to the permanent app ID's (#168).
//
// The preview ran as `com.example.note_taker_ui` (Linux), `com.example.noteTakerUi` (macOS) and
// `com.example\note_taker_ui` then `com.example\Koalino` (Windows, where the folder follows the
// company and product names). The release's ID gives the application-support folder a new name,
// so on its first start the app moves the old folder's contents into the new one, before anything
// opens them, and leaves a note saying where they went. Keychain entries use stable names and need
// no move. Android is a separate app: its data moves with a backup (#167).

import 'dart:io';

/// The note left in an old folder once its contents moved.
const String legacyMovedNoteName = 'MOVED-TO-KOALINO.txt';

/// The data folder's name inside the application-support folder.
const String _dataFolderName = 'note_taker';

/// The preview's application-support folders for `supportDirectory` on `operatingSystem`.
///
/// Inputs: The new application-support folder and `Platform.operatingSystem`.
/// Outputs: The old folders to look in, most likely first; empty where nothing moves.
/// Side effects: None.
List<Directory> legacySupportDirectoriesFor(
    Directory supportDirectory, String operatingSystem) {
  final String separator = operatingSystem == 'windows' ? r'\' : '/';
  String parentOf(String path) =>
      path.substring(0, path.lastIndexOf(separator));
  final String parent = parentOf(supportDirectory.path);
  return switch (operatingSystem) {
    'linux' => <Directory>[
        Directory('$parent${separator}com.example.note_taker_ui')
      ],
    'macos' => <Directory>[
        Directory('$parent${separator}com.example.noteTakerUi')
      ],
    'windows' => <Directory>[
        Directory(
            '${parentOf(parent)}${separator}com.example${separator}Koalino'),
        Directory(
            '${parentOf(parent)}${separator}com.example${separator}note_taker_ui'),
      ],
    _ => <Directory>[],
  };
}

/// Moves the most recently used old folder's contents into `supportDirectory`, once.
///
/// Inputs: The new application-support folder and the old ones to look in.
/// Outputs: The old folder whose contents moved, or null when nothing moved (a new install, or
/// the new folder already holds data).
/// Side effects: Creates `supportDirectory`, renames (or copies then deletes) each entry of the old
/// folder that the new one lacks, and writes `legacyMovedNoteName` into the old folder. Call it
/// before anything opens the data. A failure leaves whatever already moved in the new folder; the
/// rest is tried again at the next start.
Future<Directory?> migrateLegacyAppData({
  required Directory supportDirectory,
  required List<Directory> legacySupportDirectories,
}) async {
  if (await Directory(_child(supportDirectory, _dataFolderName)).exists()) {
    return null;
  }
  Directory? source;
  DateTime? sourceTime;
  for (final Directory legacy in legacySupportDirectories) {
    final Directory data = Directory(_child(legacy, _dataFolderName));
    if (!await data.exists()) continue;
    final File database = File(_child(data, 'note_taker.sqlite3'));
    final DateTime used = await database.exists()
        ? await database.lastModified()
        : (await data.stat()).modified;
    if (sourceTime == null || used.isAfter(sourceTime)) {
      source = legacy;
      sourceTime = used;
    }
  }
  if (source == null) return null;
  await supportDirectory.create(recursive: true);
  await for (final FileSystemEntity entry in source.list()) {
    final String name = entry.uri.pathSegments
        .lastWhere((String segment) => segment.isNotEmpty);
    if (name == legacyMovedNoteName) continue;
    final String target = _child(supportDirectory, name);
    if (await FileSystemEntity.type(target) != FileSystemEntityType.notFound) {
      continue;
    }
    await _move(entry, target);
  }
  await File(_child(source, legacyMovedNoteName)).writeAsString(
    'Koalino moved its data from this folder to:\n'
    '${supportDirectory.path}\n'
    'You can delete this folder.\n',
  );
  return source;
}

String _child(Directory parent, String name) =>
    '${parent.path}${Platform.pathSeparator}$name';

/// Renames `entry` to `target`, or copies and deletes it across file systems.
Future<void> _move(FileSystemEntity entry, String target) async {
  try {
    await entry.rename(target);
  } on FileSystemException {
    if (entry is Directory) {
      await _copyDirectory(entry, Directory(target));
    } else if (entry is File) {
      await entry.copy(target);
    } else {
      rethrow;
    }
    await entry.delete(recursive: true);
  }
}

Future<void> _copyDirectory(Directory source, Directory target) async {
  await target.create(recursive: true);
  await for (final FileSystemEntity entry in source.list()) {
    final String name = entry.uri.pathSegments
        .lastWhere((String segment) => segment.isNotEmpty);
    final String destination = _child(target, name);
    if (entry is Directory) {
      await _copyDirectory(entry, Directory(destination));
    } else if (entry is File) {
      await entry.copy(destination);
    }
  }
}
