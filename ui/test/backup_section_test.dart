// Specifies Settings' Back up all data and Restore from backup (#167) over a fake bridge: the
// passphrase is asked twice for a backup and must match, failures are worded by status, and a
// restore asks first and restarts the app only after it was staged.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/backup_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';
import 'package:note_taker_ui/presentation/backup_section.dart';

final class _FakeBridge implements BackupBridge {
  String? destination = 'content://downloads/1';
  String? backup = 'content://downloads/2';
  BackupResult createResult =
      const BackupResult(status: 0, files: 12, bytes: 3 * 1024 * 1024);
  BackupResult stageResult =
      const BackupResult(status: 0, files: 12, bytes: 1024);
  final List<String> calls = <String>[];
  String? passphrase;

  @override
  Future<String?> pickDestination(String suggestedName) async {
    calls.add('pickDestination');
    return destination;
  }

  @override
  Future<String?> pickBackup() async {
    calls.add('pickBackup');
    return backup;
  }

  @override
  Future<BackupResult> create(
      {required String uri, required String passphrase}) async {
    calls.add('create');
    this.passphrase = passphrase;
    return createResult;
  }

  @override
  Future<BackupResult> stage(
      {required String uri, required String passphrase}) async {
    calls.add('stage');
    this.passphrase = passphrase;
    return stageResult;
  }

  @override
  Future<void> restart() async => calls.add('restart');

  String? folder = 'content://tree/drive';
  AutomaticBackupStatus automatic = const AutomaticBackupStatus(enabled: false);

  @override
  Future<String?> pickFolder() async {
    calls.add('pickFolder');
    return folder;
  }

  @override
  Future<BackupResult> enableAutomatic(
      {required String folderUri, required String passphrase}) async {
    calls.add('enableAutomatic');
    this.passphrase = passphrase;
    automatic = AutomaticBackupStatus(
      enabled: true,
      folder: 'Koalino (Drive)',
      lastTime: DateTime(2026, 11, 25, 12, 30),
    );
    return const BackupResult(status: 0, files: 4);
  }

  @override
  Future<void> disableAutomatic() async {
    calls.add('disableAutomatic');
    automatic = const AutomaticBackupStatus(enabled: false);
  }

  @override
  Future<AutomaticBackupStatus> automaticStatus() async => automatic;

  @override
  Future<BackupResult> backUpNow() async {
    calls.add('backUpNow');
    return const BackupResult(status: BackupStatus.folderUnavailable);
  }
}

Future<void> _show(WidgetTester tester, BackupBridge bridge) =>
    tester.pumpWidget(MaterialApp(
      home: Scaffold(
          body: ListView(children: <Widget>[BackupSection(bridge: bridge)])),
    ));

void main() {
  testWidgets('a backup asks the passphrase twice and reports what it saved',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _show(tester, bridge);

    await tester.tap(find.byKey(backUpAllDataKey));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(backupPassphraseFieldKey), 'long passphrase');
    await tester.enterText(
        find.byKey(backupRepeatFieldKey), 'other passphrase');
    await tester.tap(find.byKey(confirmPassphraseKey));
    await tester.pumpAndSettle();
    expect(find.text(l10n.passphrasesDoNotMatch), findsOneWidget);
    expect(bridge.calls, isNot(contains('create')));

    await tester.enterText(find.byKey(backupRepeatFieldKey), 'long passphrase');
    await tester.tap(find.byKey(confirmPassphraseKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, <String>['pickDestination', 'create']);
    expect(bridge.passphrase, 'long passphrase');
    expect(find.text(l10n.backupSaved(12, '3.0')), findsOneWidget);
  });

  testWidgets('a short passphrase is refused', (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _show(tester, bridge);

    await tester.tap(find.byKey(backUpAllDataKey));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(backupPassphraseFieldKey), 'short');
    await tester.enterText(find.byKey(backupRepeatFieldKey), 'short');
    await tester.tap(find.byKey(confirmPassphraseKey));
    await tester.pumpAndSettle();

    expect(find.text(l10n.passphraseTooShort), findsOneWidget);
    expect(bridge.calls, isNot(contains('create')));
  });

  testWidgets('cancelling the passphrase opens no file picker',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _show(tester, bridge);

    await tester.tap(find.byKey(backUpAllDataKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text(l10n.cancel));
    await tester.pumpAndSettle();

    expect(bridge.calls, isEmpty);
  });

  testWidgets('a restore with a wrong passphrase says so and does not restart',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..stageResult = const BackupResult(status: BackupStatus.wrongPassphrase);
    await _show(tester, bridge);

    await tester.tap(find.byKey(restoreFromBackupKey));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(confirmRestoreKey));
    await tester.pumpAndSettle();
    await tester.enterText(find.byKey(backupPassphraseFieldKey), 'not the one');
    await tester.tap(find.byKey(confirmPassphraseKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, <String>['pickBackup', 'stage']);
    expect(find.text(l10n.backupWrongPassphrase), findsOneWidget);
  });

  testWidgets('a staged restore restarts the app', (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _show(tester, bridge);

    await tester.tap(find.byKey(restoreFromBackupKey));
    await tester.pumpAndSettle();
    await tester.tap(find.byKey(confirmRestoreKey));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(backupPassphraseFieldKey), 'long passphrase');
    await tester.tap(find.byKey(confirmPassphraseKey));
    await tester.pumpAndSettle();
    expect(find.text(l10n.restoreReady(12)), findsOneWidget);

    await tester.tap(find.byKey(restartAfterRestoreKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, <String>['pickBackup', 'stage', 'restart']);
  });

  testWidgets('turning on automatic backups asks a passphrase and a folder',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _show(tester, bridge);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(automaticBackupsSwitchKey));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(backupPassphraseFieldKey), 'long passphrase');
    await tester.enterText(find.byKey(backupRepeatFieldKey), 'long passphrase');
    await tester.tap(find.byKey(confirmPassphraseKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, <String>['pickFolder', 'enableAutomatic']);
    expect(bridge.passphrase, 'long passphrase');
    expect(find.textContaining('Koalino (Drive)'), findsOneWidget);
    expect(find.byKey(backUpNowKey), findsOneWidget);
  });

  testWidgets('a cancelled folder picker leaves automatic backups off',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()..folder = null;
    await _show(tester, bridge);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(automaticBackupsSwitchKey));
    await tester.pumpAndSettle();
    await tester.enterText(
        find.byKey(backupPassphraseFieldKey), 'long passphrase');
    await tester.enterText(find.byKey(backupRepeatFieldKey), 'long passphrase');
    await tester.tap(find.byKey(confirmPassphraseKey));
    await tester.pumpAndSettle();

    expect(bridge.calls, <String>['pickFolder']);
    expect(find.byKey(backUpNowKey), findsNothing);
  });

  testWidgets(
      'Back up now reports an unreachable folder, and the switch turns it off',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()
      ..automatic = AutomaticBackupStatus(
        enabled: true,
        folder: 'Backups (Nextcloud)',
        lastTime: DateTime(2026, 11, 24, 3),
      );
    await _show(tester, bridge);
    await tester.pumpAndSettle();

    await tester.tap(find.byKey(backUpNowKey));
    await tester.pumpAndSettle();
    expect(find.text(l10n.backupFolderUnavailable), findsOneWidget);

    await tester.tap(find.byKey(automaticBackupsSwitchKey));
    await tester.pumpAndSettle();
    expect(bridge.calls, <String>['backUpNow', 'disableAutomatic']);
    expect(find.byKey(backUpNowKey), findsNothing);
  });
}
