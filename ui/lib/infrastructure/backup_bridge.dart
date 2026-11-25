// The app's side of backing up and restoring all data (#167), on Android.
//
// `AndroidBackupBridge` talks to `BackupChannel.kt` over `note_taker/backup`. The database key
// never passes through Dart: Kotlin unwraps it for a backup and wraps the restored one itself.
// The passphrase does pass through, because the owner types it here; it is sent once and not
// kept. Settings uses the `BackupBridge` interface, so its widget tests run without a device.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:path_provider/path_provider.dart';

import 'app_configuration.dart';

/// The core's statuses for a backup or restore (`core/bin/backup_commands.rs`).
abstract final class BackupStatus {
  /// Done.
  static const int success = 0;

  /// The file is not a Koalino backup.
  static const int notABackup = 21;

  /// The backup comes from a newer version.
  static const int unsupportedVersion = 22;

  /// The passphrase is wrong.
  static const int wrongPassphrase = 23;

  /// The backup is damaged or incomplete.
  static const int damaged = 24;

  /// The backup's key does not open its database.
  static const int keyMismatch = 26;

  /// Automatic backups are off, or their stored passphrase is gone (#170).
  static const int notSetUp = 30;

  /// The automatic backups' folder could not be written (#170).
  static const int folderUnavailable = 31;

  /// A failure outside the core (opening the file, the Keystore).
  static const int platformFailure = -1;
}

/// What a backup or restore did.
final class BackupResult {
  /// Creates a result.
  const BackupResult({required this.status, this.files = 0, this.bytes = 0});

  /// Reads Kotlin's `{status, files, bytes}`.
  factory BackupResult.fromMap(Map<Object?, Object?> map) => BackupResult(
        status:
            (map['status'] as num?)?.toInt() ?? BackupStatus.platformFailure,
        files: (map['files'] as num?)?.toInt() ?? 0,
        bytes: (map['bytes'] as num?)?.toInt() ?? 0,
      );

  /// `BackupStatus.success` or the failure.
  final int status;

  /// Files written or restored.
  final int files;

  /// Their clear size in bytes.
  final int bytes;

  /// Whether it worked.
  bool get succeeded => status == BackupStatus.success;
}

/// The state of automatic backups (#170).
final class AutomaticBackupStatus {
  /// Creates a status.
  const AutomaticBackupStatus({
    required this.enabled,
    this.folder,
    this.lastTime,
    this.lastStatus = BackupStatus.success,
    this.lastFiles = 0,
  });

  /// Reads Kotlin's `AutomaticBackup.status()`.
  factory AutomaticBackupStatus.fromMap(Map<Object?, Object?> map) {
    final int? lastMillis = (map['lastTime'] as num?)?.toInt();
    return AutomaticBackupStatus(
      enabled: map['enabled'] as bool? ?? false,
      folder: map['folder'] as String?,
      lastTime: lastMillis == null
          ? null
          : DateTime.fromMillisecondsSinceEpoch(lastMillis),
      lastStatus: (map['lastStatus'] as num?)?.toInt() ?? BackupStatus.success,
      lastFiles: (map['lastFiles'] as num?)?.toInt() ?? 0,
    );
  }

  /// Whether the daily job is on.
  final bool enabled;

  /// The folder's name and its provider, for example "Koalino (Drive)".
  final String? folder;

  /// When the last automatic backup ran, or null before the first.
  final DateTime? lastTime;

  /// That run's `BackupStatus`.
  final int lastStatus;

  /// Files that run wrote.
  final int lastFiles;
}

/// Backs up and restores all data; implemented on Android.
abstract interface class BackupBridge {
  /// Asks where to save a new backup. Outputs: the document's URI, or null when cancelled.
  Future<String?> pickDestination(String suggestedName);

  /// Asks which backup to restore. Outputs: the document's URI, or null when cancelled.
  Future<String?> pickBackup();

  /// Writes a backup of all data to `uri`, sealed with `passphrase`.
  ///
  /// Side effects: Reads the data folder; takes seconds to minutes.
  Future<BackupResult> create(
      {required String uri, required String passphrase});

  /// Restores the backup at `uri` into the staging folder; it takes effect after `restart`.
  ///
  /// Side effects: Writes the staging folder (removed again on failure).
  Future<BackupResult> stage({required String uri, required String passphrase});

  /// Closes and reopens the app, which swaps a staged restore in.
  Future<void> restart();

  /// Asks for the automatic backups' folder, local or at a cloud provider (#170).
  /// Outputs: the folder's tree URI, or null when cancelled.
  Future<String?> pickFolder();

  /// Turns automatic backups on into `folderUri`, sealed with `passphrase`, and runs the first.
  ///
  /// Side effects: Keeps the passphrase on the device, wrapped by the Keystore; schedules the
  /// daily job.
  Future<BackupResult> enableAutomatic(
      {required String folderUri, required String passphrase});

  /// Turns automatic backups off; the backups already written stay.
  Future<void> disableAutomatic();

  /// Reads whether automatic backups are on and how the last one went.
  Future<AutomaticBackupStatus> automaticStatus();

  /// Writes an automatic backup now.
  Future<BackupResult> backUpNow();
}

/// `BackupBridge` over `BackupChannel.kt`.
final class AndroidBackupBridge implements BackupBridge {
  /// Creates the bridge.
  const AndroidBackupBridge();

  static const MethodChannel _channel = MethodChannel('note_taker/backup');

  @override
  Future<String?> pickDestination(String suggestedName) =>
      _channel.invokeMethod<String>(
          'pickDestination', <String, Object?>{'suggestedName': suggestedName});

  @override
  Future<String?> pickBackup() => _channel.invokeMethod<String>('pickBackup');

  @override
  Future<BackupResult> create(
      {required String uri, required String passphrase}) async {
    final Directory data = await appDataDirectory();
    return _call('create', <String, Object?>{
      'uri': uri,
      'passphrase': passphrase,
      'dataDirectory': data.path,
      'databaseName': 'note_taker.sqlite3',
      'keyDirectory': await appKeyDirectory(),
    });
  }

  @override
  Future<BackupResult> stage(
      {required String uri, required String passphrase}) async {
    final Directory support = await getApplicationSupportDirectory();
    final Directory staging =
        Directory('${support.path}${Platform.pathSeparator}note_taker.restore');
    // A staging folder left by an earlier attempt that was never applied.
    if (await staging.exists()) {
      await staging.delete(recursive: true);
    }
    return _call('stage', <String, Object?>{
      'uri': uri,
      'passphrase': passphrase,
      'stagingDirectory': staging.path,
    });
  }

  @override
  Future<void> restart() => _channel.invokeMethod<void>('restart');

  @override
  Future<String?> pickFolder() async {
    try {
      return await _channel.invokeMethod<String>('pickFolder');
    } on PlatformException {
      return null;
    }
  }

  @override
  Future<BackupResult> enableAutomatic(
          {required String folderUri, required String passphrase}) async =>
      _call('enableAutomatic', <String, Object?>{
        'uri': folderUri,
        'passphrase': passphrase,
        'dataDirectory': (await appDataDirectory()).path,
        'keyDirectory': await appKeyDirectory(),
      });

  @override
  Future<void> disableAutomatic() =>
      _channel.invokeMethod<void>('disableAutomatic');

  @override
  Future<AutomaticBackupStatus> automaticStatus() async {
    try {
      final Map<Object?, Object?>? answer =
          await _channel.invokeMethod<Map<Object?, Object?>>('automaticStatus');
      return answer == null
          ? const AutomaticBackupStatus(enabled: false)
          : AutomaticBackupStatus.fromMap(answer);
    } on PlatformException {
      return const AutomaticBackupStatus(enabled: false);
    }
  }

  @override
  Future<BackupResult> backUpNow() =>
      _call('backUpNow', const <String, Object?>{});

  Future<BackupResult> _call(
      String method, Map<String, Object?> arguments) async {
    try {
      final Map<Object?, Object?>? answer =
          await _channel.invokeMethod<Map<Object?, Object?>>(method, arguments);
      return answer == null
          ? const BackupResult(status: BackupStatus.platformFailure)
          : BackupResult.fromMap(answer);
    } on PlatformException {
      return const BackupResult(status: BackupStatus.platformFailure);
    }
  }
}
