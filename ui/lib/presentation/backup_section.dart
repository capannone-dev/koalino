// Settings' Back up all data and Restore from backup (#167), on Android.
//
// A backup is one encrypted file with the meetings, notes, attachments, kept recordings and the
// key that opens them, sealed with a passphrase the owner chooses. It lets the owner move to
// another phone or to a reinstalled app. The widgets stay dumb: the file pickers, the key and
// the format live in Kotlin and the core, reached through `BackupBridge`. Automatic backups
// (#170) write the same file daily, while charging, into a folder the owner picks, which may be
// at a cloud provider; the file is encrypted before it leaves the phone.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/backup_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Key of the Back up all data tile.
const Key backUpAllDataKey = ValueKey<String>('back-up-all-data');

/// Key of the Restore from backup tile.
const Key restoreFromBackupKey = ValueKey<String>('restore-from-backup');

/// Key of the passphrase field.
const Key backupPassphraseFieldKey = ValueKey<String>('backup-passphrase');

/// Key of the repeated passphrase field (backups only).
const Key backupRepeatFieldKey = ValueKey<String>('backup-passphrase-repeat');

/// Key of the passphrase dialog's confirm button.
const Key confirmPassphraseKey = ValueKey<String>('confirm-backup-passphrase');

/// Key of the restore warning's confirm button.
const Key confirmRestoreKey = ValueKey<String>('confirm-restore');

/// Key of the Restart button after a restore was staged.
const Key restartAfterRestoreKey = ValueKey<String>('restart-after-restore');

/// Key of the Automatic backups switch (#170).
const Key automaticBackupsSwitchKey = ValueKey<String>('automatic-backups');

/// Key of the Back up now button (#170).
const Key backUpNowKey = ValueKey<String>('back-up-now');

/// The shortest passphrase accepted for a new backup.
const int minimumBackupPassphraseLength = 8;

/// Words a backup or restore failure for the owner.
///
/// Inputs: A `BackupStatus`. Outputs: The message. Side effects: None.
String backupFailureMessage(int status) => switch (status) {
      BackupStatus.notABackup => l10n.backupNotABackup,
      BackupStatus.unsupportedVersion => l10n.backupTooNew,
      BackupStatus.wrongPassphrase => l10n.backupWrongPassphrase,
      BackupStatus.damaged || BackupStatus.keyMismatch => l10n.backupDamaged,
      BackupStatus.folderUnavailable => l10n.backupFolderUnavailable,
      BackupStatus.notSetUp => l10n.automaticBackupsNotSetUp,
      _ => l10n.backupFailed,
    };

/// The Settings tiles (back up, restore, automatic backups) and their progress.
final class BackupSection extends StatefulWidget {
  /// Creates the section over `bridge`.
  ///
  /// Side effects: None until a tile is tapped.
  const BackupSection({required this.bridge, super.key});

  final BackupBridge bridge;

  @override
  State<BackupSection> createState() => _BackupSectionState();
}

final class _BackupSectionState extends State<BackupSection> {
  /// The running step's wording, or null when idle.
  String? _busy;

  /// The last outcome, shown under the tiles.
  String? _message;

  /// Automatic backups' state (#170), null until read.
  AutomaticBackupStatus? _automatic;

  @override
  void initState() {
    super.initState();
    unawaited(_loadAutomatic());
  }

  Future<void> _loadAutomatic() async {
    final AutomaticBackupStatus status = await widget.bridge.automaticStatus();
    if (mounted) setState(() => _automatic = status);
  }

  /// Turns automatic backups on (passphrase, then folder, then a first backup) or off.
  Future<void> _setAutomatic(bool on) async {
    if (!on) {
      await widget.bridge.disableAutomatic();
      await _loadAutomatic();
      return;
    }
    final String? passphrase = await _askPassphrase(repeat: true);
    if (passphrase == null || !mounted) return;
    final String? folder = await widget.bridge.pickFolder();
    if (folder == null || !mounted) return;
    setState(() {
      _busy = l10n.backingUp;
      _message = null;
    });
    final BackupResult result = await widget.bridge
        .enableAutomatic(folderUri: folder, passphrase: passphrase);
    await _loadAutomatic();
    if (!mounted) return;
    setState(() {
      _busy = null;
      _message = result.succeeded ? null : backupFailureMessage(result.status);
    });
  }

  Future<void> _backUpNow() async {
    setState(() {
      _busy = l10n.backingUp;
      _message = null;
    });
    final BackupResult result = await widget.bridge.backUpNow();
    await _loadAutomatic();
    if (!mounted) return;
    setState(() {
      _busy = null;
      _message = result.succeeded ? null : backupFailureMessage(result.status);
    });
  }

  /// Words the folder and the last automatic backup.
  String _automaticSummary(AutomaticBackupStatus status) {
    final DateTime? last = status.lastTime;
    final String lastRun = last == null
        ? l10n.noAutomaticBackupYet
        : status.lastStatus == BackupStatus.success
            ? l10n.lastAutomaticBackup(_when(last))
            : l10n.lastAutomaticBackupFailed(_when(last));
    return '${l10n.automaticBackupFolder(status.folder ?? '?')}\n$lastRun';
  }

  String _when(DateTime time) =>
      '${time.year}-${_two(time.month)}-${_two(time.day)} '
      '${_two(time.hour)}:${_two(time.minute)}';

  String _two(int value) => value.toString().padLeft(2, '0');

  Future<void> _backUp() async {
    final DateTime now = DateTime.now();
    final String day = '${now.year}-${now.month.toString().padLeft(2, '0')}-'
        '${now.day.toString().padLeft(2, '0')}';
    // The passphrase comes first: the picker creates the file, so cancelling after it would
    // leave an empty one behind.
    final String? passphrase = await _askPassphrase(repeat: true);
    if (passphrase == null || !mounted) return;
    final String? uri =
        await widget.bridge.pickDestination('koalino-$day.koalino-backup');
    if (uri == null || !mounted) return;
    setState(() {
      _busy = l10n.backingUp;
      _message = null;
    });
    final BackupResult result =
        await widget.bridge.create(uri: uri, passphrase: passphrase);
    if (!mounted) return;
    setState(() {
      _busy = null;
      _message = result.succeeded
          ? l10n.backupSaved(
              result.files, (result.bytes / (1024 * 1024)).toStringAsFixed(1))
          : backupFailureMessage(result.status);
    });
  }

  Future<void> _restore() async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.restoreConfirmTitle),
        content: SingleChildScrollView(child: Text(l10n.restoreConfirmBody)),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            key: confirmRestoreKey,
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l10n.restoreAction),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    final String? uri = await widget.bridge.pickBackup();
    if (uri == null || !mounted) return;
    final String? passphrase = await _askPassphrase(repeat: false);
    if (passphrase == null || !mounted) return;
    setState(() {
      _busy = l10n.restoringBackup;
      _message = null;
    });
    final BackupResult result =
        await widget.bridge.stage(uri: uri, passphrase: passphrase);
    if (!mounted) return;
    setState(() {
      _busy = null;
      _message = result.succeeded ? null : backupFailureMessage(result.status);
    });
    if (!result.succeeded) return;
    await showDialog<void>(
      context: context,
      barrierDismissible: false,
      builder: (BuildContext context) => AlertDialog(
        content: Text(l10n.restoreReady(result.files)),
        actions: <Widget>[
          FilledButton(
            key: restartAfterRestoreKey,
            onPressed: () => Navigator.of(context).pop(),
            child: Text(l10n.restartNow),
          ),
        ],
      ),
    );
    await widget.bridge.restart();
  }

  /// Asks for the passphrase; `repeat` adds the second field and the length rule of a new backup.
  Future<String?> _askPassphrase({required bool repeat}) => showDialog<String>(
        context: context,
        builder: (BuildContext context) => _PassphraseDialog(repeat: repeat),
      );

  @override
  Widget build(BuildContext context) {
    final bool idle = _busy == null;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        ListTile(
          key: backUpAllDataKey,
          leading: const Icon(Icons.backup_outlined),
          title: Text(l10n.backUpAllData),
          subtitle: Text(l10n.backUpAllDataDescription),
          enabled: idle,
          onTap: _backUp,
        ),
        ListTile(
          key: restoreFromBackupKey,
          leading: const Icon(Icons.settings_backup_restore),
          title: Text(l10n.restoreFromBackup),
          subtitle: Text(l10n.restoreFromBackupDescription),
          enabled: idle,
          onTap: _restore,
        ),
        SwitchListTile(
          key: automaticBackupsSwitchKey,
          secondary: const Icon(Icons.cloud_sync_outlined),
          title: Text(l10n.automaticBackups),
          subtitle: Text(l10n.automaticBackupsDescription),
          value: _automatic?.enabled ?? false,
          onChanged: idle && _automatic != null ? _setAutomatic : null,
        ),
        if (_automatic case final AutomaticBackupStatus automatic
            when automatic.enabled)
          Padding(
            padding: const EdgeInsets.fromLTRB(72, 0, 16, 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(_automaticSummary(automatic)),
                const SizedBox(height: 8),
                OutlinedButton.icon(
                  key: backUpNowKey,
                  onPressed: idle ? _backUpNow : null,
                  icon: const Icon(Icons.backup_outlined),
                  label: Text(l10n.backUpNow),
                ),
              ],
            ),
          ),
        if (_busy case final String busy)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(busy),
                const SizedBox(height: 8),
                const LinearProgressIndicator(),
              ],
            ),
          ),
        if (_message case final String message)
          Padding(
            padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
            child: Text(message),
          ),
      ],
    );
  }
}

/// Asks for a passphrase, twice for a new backup; pops it, or null when cancelled.
final class _PassphraseDialog extends StatefulWidget {
  const _PassphraseDialog({required this.repeat});

  final bool repeat;

  @override
  State<_PassphraseDialog> createState() => _PassphraseDialogState();
}

final class _PassphraseDialogState extends State<_PassphraseDialog> {
  final TextEditingController _passphrase = TextEditingController();
  final TextEditingController _repeat = TextEditingController();
  String? _error;

  @override
  void dispose() {
    _passphrase.dispose();
    _repeat.dispose();
    super.dispose();
  }

  void _confirm() {
    final String passphrase = _passphrase.text;
    final String? error = switch (passphrase) {
      '' => l10n.passphraseTooShort,
      _
          when widget.repeat &&
              passphrase.length < minimumBackupPassphraseLength =>
        l10n.passphraseTooShort,
      _ when widget.repeat && passphrase != _repeat.text =>
        l10n.passphrasesDoNotMatch,
      _ => null,
    };
    if (error != null) {
      setState(() => _error = error);
      return;
    }
    Navigator.of(context).pop(passphrase);
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: Text(l10n.backupPassphraseTitle),
        content: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              if (widget.repeat) Text(l10n.backupPassphraseExplanation),
              TextField(
                key: backupPassphraseFieldKey,
                controller: _passphrase,
                obscureText: true,
                autocorrect: false,
                enableSuggestions: false,
                autofocus: true,
                decoration:
                    InputDecoration(labelText: l10n.backupPassphraseLabel),
              ),
              if (widget.repeat)
                TextField(
                  key: backupRepeatFieldKey,
                  controller: _repeat,
                  obscureText: true,
                  autocorrect: false,
                  enableSuggestions: false,
                  decoration:
                      InputDecoration(labelText: l10n.repeatPassphraseLabel),
                ),
              if (_error case final String error)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    error,
                    style:
                        TextStyle(color: Theme.of(context).colorScheme.error),
                  ),
                ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            key: confirmPassphraseKey,
            onPressed: _confirm,
            child: Text(l10n.continueAction),
          ),
        ],
      );
}
