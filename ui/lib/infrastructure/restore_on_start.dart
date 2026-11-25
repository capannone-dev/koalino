// Applies a restored backup at start-up, before the core opens any data (#167).
//
// Restoring writes the backup into `<support>/note_taker.restore` while the app runs
// (`BackupChannel.kt`), then restarts the app. On the next start, before `configureCore`, this
// swaps that folder in: the current data moves aside to `note_taker.before-restore` (replacing
// the copy an earlier restore kept), and models already on the device, which a backup leaves
// out, move into the restored folder. A staging folder without the ready marker comes from an
// interrupted restore and is deleted, leaving the data untouched.

import 'dart:io';

/// The file `BackupChannel.kt` writes once the staged restore and its key are complete.
const String restoreReadyMarker = '.restore-ready';

/// The data folder's name under the application-support folder.
const String _dataFolderName = 'note_taker';

/// Where a restore is staged.
const String _stagingFolderName = 'note_taker.restore';

/// Where the data replaced by the last restore is kept.
const String _previousFolderName = 'note_taker.before-restore';

/// What `applyPendingRestore` did.
enum PendingRestore {
  /// Nothing was staged.
  none,

  /// A ready restore replaced the data.
  applied,

  /// An unfinished restore was deleted; the data is unchanged.
  discarded,
}

/// Swaps a staged restore in, if one is ready.
///
/// Inputs: The application-support folder that holds `note_taker`.
/// Outputs: What was done. Throws a `FileSystemException` when a rename fails; the data folder
/// is put back first when it had already moved aside.
/// Side effects: Renames and deletes folders under `supportDirectory`; call it before the core
/// opens the database.
Future<PendingRestore> applyPendingRestore(Directory supportDirectory) async {
  String path(String name) =>
      '${supportDirectory.path}${Platform.pathSeparator}$name';
  final Directory staging = Directory(path(_stagingFolderName));
  if (!await staging.exists()) {
    return PendingRestore.none;
  }
  final File marker =
      File('${staging.path}${Platform.pathSeparator}$restoreReadyMarker');
  if (!await marker.exists()) {
    await staging.delete(recursive: true);
    return PendingRestore.discarded;
  }
  final Directory data = Directory(path(_dataFolderName));
  final Directory previous = Directory(path(_previousFolderName));
  if (await previous.exists()) {
    await previous.delete(recursive: true);
  }
  final bool hadData = await data.exists();
  if (hadData) {
    await data.rename(previous.path);
  }
  try {
    await staging.rename(data.path);
  } on FileSystemException {
    if (hadData) {
      await previous.rename(data.path);
    }
    rethrow;
  }
  await File('${data.path}${Platform.pathSeparator}$restoreReadyMarker')
      .delete();
  final Directory oldModels =
      Directory('${previous.path}${Platform.pathSeparator}models');
  final Directory newModels =
      Directory('${data.path}${Platform.pathSeparator}models');
  if (hadData && await oldModels.exists() && !await newModels.exists()) {
    await oldModels.rename(newModels.path);
  }
  return PendingRestore.applied;
}
