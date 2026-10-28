// Specifies the Settings screen's diagnostics (Issue #46): reading and changing the log level,
// exporting the logs through the desktop save dialog, sharing them on Android, the error
// messages, and the screen on platforms without App lock.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/log_bridge.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'fake_log_bridge.dart';

void main() {
  late Directory root;

  setUp(() => root = Directory.systemTemp.createTempSync('log-settings-'));
  tearDown(() => root.deleteSync(recursive: true));

  /// The time every export is stamped with.
  DateTime fixedNow() => DateTime(2026, 10, 28, 9, 5, 7);

  /// A desktop controller (no sharer) over `bridge`.
  LogSettingsController desktopController(FakeLogBridge bridge,
          {String? chosenPath}) =>
      LogSettingsController(
        logBridge: bridge,
        exportSaver: FakeDesktopSaver(chosenPath: chosenPath),
        sharedDirectory: () async => Directory('${root.path}/shared'),
        now: fixedNow,
      );

  group('log level', () {
    test('decodes the core level, and unknown directives as none', () {
      expect(LogLevel.fromDirective('debug'), LogLevel.debug);
      expect(LogLevel.fromDirective(' WARN '), LogLevel.warn);
      expect(LogLevel.fromDirective('note_taker=trace,info'), isNull);
      expect(LogLevel.values.map((LogLevel level) => level.directive),
          <String>['error', 'warn', 'info', 'debug', 'trace']);
    });

    test('loads the stored level', () async {
      final LogSettingsController controller =
          desktopController(FakeLogBridge(storedLevel: 'debug'));

      await controller.load();

      expect(controller.level, LogLevel.debug);
    });

    test('a new level is sent to the core and shown', () async {
      final FakeLogBridge bridge = FakeLogBridge();
      final LogSettingsController controller = desktopController(bridge);
      await controller.load();

      await controller.setLevel(LogLevel.trace);

      expect(bridge.requestedLevels, <String>['trace']);
      expect(controller.level, LogLevel.trace);
      expect(controller.levelMessage, isNull);
    });

    test('a refused level keeps the old one and explains', () async {
      final LogSettingsController controller =
          desktopController(FakeLogBridge(acceptsLevels: false));
      await controller.load();

      await controller.setLevel(LogLevel.debug);

      expect(controller.level, LogLevel.info);
      expect(controller.levelMessage, 'Unable to change the log level');
    });
  });

  group('desktop export', () {
    test('the core writes where the owner chose', () async {
      final FakeLogBridge bridge = FakeLogBridge();
      final String chosen = '${root.path}/my logs.jsonl';
      final FakeDesktopSaver saver = FakeDesktopSaver(chosenPath: chosen);
      final LogSettingsController controller = LogSettingsController(
        logBridge: bridge,
        exportSaver: saver,
        sharedDirectory: () async => Directory('${root.path}/shared'),
        now: fixedNow,
      );

      final String? message = await controller.exportLogs();

      expect(saver.suggestedNames,
          <String>['note_taker_logs_20261028-090507.jsonl']);
      expect(bridge.exportPaths, <String?>[chosen]);
      expect(message, 'Logs exported to $chosen');
      expect(controller.isExporting, isFalse);
    });

    test('a cancelled dialog exports nothing', () async {
      final FakeLogBridge bridge = FakeLogBridge();

      final String? message = await desktopController(bridge).exportLogs();

      expect(message, isNull);
      expect(bridge.exportPaths, isEmpty);
    });

    test('no logs yet is explained', () async {
      final String? message = await desktopController(
              FakeLogBridge(
                  exportFailure: const LogExportFailure(errorCode: 'no_logs')),
              chosenPath: '${root.path}/logs.jsonl')
          .exportLogs();

      expect(message, 'No logs to export yet');
    });

    test('any other failure is worded without details', () async {
      final String? message = await desktopController(
              FakeLogBridge(
                  exportFailure:
                      const LogExportFailure(errorCode: 'export_failed')),
              chosenPath: '${root.path}/logs.jsonl')
          .exportLogs();

      expect(message, 'Unable to export the logs');
    });
  });

  group('Android share', () {
    /// An Android controller over `bridge` and `sharer`.
    LogSettingsController shareController(
            FakeLogBridge bridge, FakeLogSharer sharer) =>
        LogSettingsController(
          logBridge: bridge,
          exportSaver: FakeDesktopSaver(),
          logSharer: sharer,
          sharedDirectory: () async => Directory('${root.path}/shared'),
          now: fixedNow,
        );

    test('the export goes to the share sheet from the shared folder', () async {
      final FakeLogBridge bridge = FakeLogBridge();
      final FakeLogSharer sharer = FakeLogSharer();
      final File earlier = File('${root.path}/shared/old.jsonl')
        ..createSync(recursive: true);

      final String? message =
          await shareController(bridge, sharer).exportLogs();

      final String expected =
          '${root.path}/shared/note_taker_logs_20261028-090507.jsonl';
      expect(bridge.exportPaths, <String?>[expected]);
      expect(sharer.sharedPaths, <String>[expected]);
      expect(message, isNull);
      expect(earlier.existsSync(), isFalse,
          reason: 'an earlier shared export is removed');
    });

    test('a share sheet that fails is explained', () async {
      final String? message =
          await shareController(FakeLogBridge(), FakeLogSharer(fails: true))
              .exportLogs();

      expect(message, 'Unable to share the logs');
    });

    test('no logs yet is explained and nothing is shared', () async {
      final FakeLogSharer sharer = FakeLogSharer();

      final String? message = await shareController(
              FakeLogBridge(
                  exportFailure: const LogExportFailure(errorCode: 'no_logs')),
              sharer)
          .exportLogs();

      expect(message, 'No logs to export yet');
      expect(sharer.sharedPaths, isEmpty);
    });
  });

  test('decodes the core export answer', () {
    expect(
        logExportResultFromJson(
            '{"ok":true,"file_path":"/tmp/l.jsonl","size_bytes":5,"file_count":1}'),
        const LogExportSuccess(filePath: '/tmp/l.jsonl', sizeBytes: 5));
    expect(
        logExportResultFromJson(
            '{"ok":false,"error_code":"no_logs","error":"none"}'),
        const LogExportFailure(errorCode: 'no_logs'));
    expect(logExportResultFromJson(null),
        const LogExportFailure(errorCode: 'runtime_unavailable'));
    expect(logExportResultFromJson('not json'),
        const LogExportFailure(errorCode: 'invalid_response'));
  });

  group('SettingsScreen', () {
    testWidgets('without App lock it shows the diagnostics only',
        (WidgetTester tester) async {
      final LogSettingsController controller =
          desktopController(FakeLogBridge());
      await controller.load();

      await tester.pumpWidget(
          MaterialApp(home: SettingsScreen(logSettingsController: controller)));

      expect(find.text('App lock'), findsNothing);
      expect(find.text('Log level'), findsOneWidget);
      expect(find.text('Info (default)'), findsOneWidget);
      expect(find.text('Export logs'), findsOneWidget);
    });

    testWidgets('choosing a level changes it', (WidgetTester tester) async {
      final FakeLogBridge bridge = FakeLogBridge();
      final LogSettingsController controller = desktopController(bridge);
      await controller.load();
      await tester.pumpWidget(
          MaterialApp(home: SettingsScreen(logSettingsController: controller)));

      await tester.tap(find.text('Info (default)'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Debug').last);
      await tester.pumpAndSettle();

      expect(bridge.requestedLevels, <String>['debug']);
      expect(find.text('Debug'), findsOneWidget);
    });

    testWidgets('Export logs reports where the logs went',
        (WidgetTester tester) async {
      final String chosen = '${root.path}/logs.jsonl';
      // The desktop flow does no file I/O of its own, so it runs in the test's fake time.
      final LogSettingsController controller = desktopController(
          FakeLogBridge(
              exportFailure: const LogExportFailure(errorCode: 'no_logs')),
          chosenPath: chosen);
      await controller.load();
      await tester.pumpWidget(
          MaterialApp(home: SettingsScreen(logSettingsController: controller)));

      await tester.tap(find.text('Export logs'));
      await tester.pumpAndSettle();

      expect(find.text('No logs to export yet'), findsOneWidget);
    });

    testWidgets('on Android the action shares the logs',
        (WidgetTester tester) async {
      final LogSettingsController controller = LogSettingsController(
        logBridge: FakeLogBridge(),
        exportSaver: FakeDesktopSaver(),
        logSharer: FakeLogSharer(),
        sharedDirectory: () async => Directory('${root.path}/shared'),
        now: fixedNow,
      );
      await controller.load();

      await tester.pumpWidget(
          MaterialApp(home: SettingsScreen(logSettingsController: controller)));

      expect(find.text('Share logs'), findsOneWidget);
      expect(find.text('Export logs'), findsNothing);
    });
  });
}
