// Specifies exporting a meeting from the app (Issue #48): decoding Rust's result, the suggested
// file name, the desktop flow (the core writes where the owner chose, or to the default export
// folder when no save dialog can open), the mobile flow (the core writes a private file whose
// bytes go to the system save dialog, then it is deleted), errors, and the meeting's Export menu.

import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_export_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';

import 'fake_meeting_bridge.dart';

/// One export request received by the fake bridge.
typedef ExportRequest = ({
  String meetingId,
  MeetingExportFormat format,
  String? outputPath,
  bool includeAttachments,
});

/// Writes a small file where the core would, or fails with `failure`.
final class FakeExportBridge implements ExportBridge {
  /// Creates a bridge writing into `defaultDirectory` when no output path is given.
  FakeExportBridge(
      {required this.defaultDirectory,
      this.failure,
      this.attachmentNames = const <String>[]});

  final Directory defaultDirectory;

  /// Attachments a Markdown export copies into `<stem>_attachments/` beside the file.
  final List<String> attachmentNames;

  /// When set, every export fails with it.
  final MeetingExportResult? failure;

  final List<ExportRequest> requests = <ExportRequest>[];

  @override
  Future<MeetingExportResult> exportMeeting(
    String meetingId,
    MeetingExportFormat format, {
    String? outputPath,
    bool includeAttachments = true,
  }) async {
    requests.add((
      meetingId: meetingId,
      format: format,
      outputPath: outputPath,
      includeAttachments: includeAttachments,
    ));
    final MeetingExportResult? configuredFailure = failure;
    if (configuredFailure != null) {
      return configuredFailure;
    }
    final String path = outputPath ??
        '${defaultDirectory.path}/$meetingId.${format.fileExtension}';
    File(path).writeAsStringSync('exported $meetingId');
    if (format == MeetingExportFormat.markdown &&
        includeAttachments &&
        attachmentNames.isNotEmpty) {
      final String stem = path.substring(0, path.length - '.md'.length);
      final Directory folder = Directory('${stem}_attachments')..createSync();
      for (final String name in attachmentNames) {
        File('${folder.path}/$name').writeAsStringSync(name);
      }
    }
    return MeetingExportSuccess(filePath: path, sizeBytes: 12);
  }
}

/// Answers save dialogs from a script and records what it was asked.
final class FakeExportSaver implements ExportSaver {
  FakeExportSaver({
    required this.savesBytes,
    this.chosenPath,
    this.dialogFails = false,
    this.bytesSaved = true,
  });

  @override
  final bool savesBytes;

  /// The path the desktop dialog returns (`null`: cancelled).
  final String? chosenPath;

  /// Whether the desktop dialog cannot open (no zenity, for example).
  final bool dialogFails;

  /// Whether the mobile dialog saves (false: cancelled).
  final bool bytesSaved;

  final List<String> suggestedNames = <String>[];
  Uint8List? savedBytes;

  @override
  Future<String?> chooseSavePath(String suggestedFileName) async {
    suggestedNames.add(suggestedFileName);
    if (dialogFails) {
      throw const FileSystemException('no save dialog available');
    }
    return chosenPath;
  }

  @override
  Future<bool> saveBytes(String suggestedFileName, Uint8List bytes) async {
    suggestedNames.add(suggestedFileName);
    if (bytesSaved) {
      savedBytes = bytes;
    }
    return bytesSaved;
  }
}

/// Records what would go to the share sheet.
final class FakeExportSharer implements ExportSharer {
  /// Creates a sharer whose folder is `directory`.
  FakeExportSharer(this.directory, {this.fails = false});

  final Directory directory;

  /// Whether the share sheet cannot open.
  final bool fails;

  /// The names of the shared files, and the text, per share.
  final List<(List<String>, String)> shares = <(List<String>, String)>[];

  @override
  Future<String> shareDirectory() async => directory.path;

  @override
  Future<void> shareFiles(List<String> paths, String text) async {
    if (fails) {
      throw const FileSystemException('no share sheet');
    }
    for (final String path in paths) {
      expect(File(path).parent.path, directory.path,
          reason: 'only files directly in the share folder can be served');
    }
    shares.add((
      <String>[for (final String path in paths) path.split('/').last],
      text
    ));
  }
}

void main() {
  late Directory root;

  setUp(() => root = Directory.systemTemp.createTempSync('meeting-export-'));
  tearDown(() => root.deleteSync(recursive: true));

  group('result decoding', () {
    test('a success carries the written file', () {
      expect(
        meetingExportResultFromJson(
            '{"ok":true,"file_path":"/tmp/a.pdf","format":"pdf","size_bytes":1234,"metadata":{}}'),
        const MeetingExportSuccess(filePath: '/tmp/a.pdf', sizeBytes: 1234),
      );
    });

    test('a failure carries its error code', () {
      expect(
        meetingExportResultFromJson(
            '{"ok":false,"error_code":"meeting_not_found","error":"missing"}'),
        const MeetingExportFailure(errorCode: 'meeting_not_found'),
      );
    });

    test('a missing or malformed answer is a failure', () {
      expect(meetingExportResultFromJson(null),
          const MeetingExportFailure(errorCode: 'runtime_unavailable'));
      expect(meetingExportResultFromJson('not json'),
          const MeetingExportFailure(errorCode: 'invalid_response'));
    });
  });

  test('the suggested file name is the title, made safe for every platform',
      () {
    expect(suggestedExportFileName('Weekly sync', MeetingExportFormat.pdf),
        'Weekly sync.pdf');
    expect(
        suggestedExportFileName(
            'Q3: plan/review?', MeetingExportFormat.markdown),
        'Q3_ plan_review_.md');
    expect(suggestedExportFileName('  ', MeetingExportFormat.json),
        'meeting.json');
  });

  group('several meetings (#216)', () {
    test('a desktop writes each to the export folder, with no dialog',
        () async {
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final FakeExportSaver saver = FakeExportSaver(savesBytes: false);
      final MeetingExportController controller =
          MeetingExportController(exportBridge: bridge, exportSaver: saver);

      final String? message =
          await controller.exportMeetings(<({String meetingId, String title})>[
        (meetingId: 'm1', title: 'Weekly sync'),
        (meetingId: 'm2', title: 'Budget'),
      ], MeetingExportFormat.pdf);

      expect(saver.suggestedNames, isEmpty);
      expect(bridge.requests.map((ExportRequest request) => request.outputPath),
          <String?>[null, null]);
      expect(message, 'Exported 2 meetings to ${root.path}');
    });

    test('Android shares them all in one sheet, even with the same title',
        () async {
      final Directory shareFolder = Directory('${root.path}/share')
        ..createSync();
      final FakeExportSharer sharer = FakeExportSharer(shareFolder);
      final MeetingExportController controller = MeetingExportController(
          exportBridge: FakeExportBridge(defaultDirectory: root),
          exportSaver: FakeExportSaver(savesBytes: true),
          exportSharer: sharer);

      final String? message =
          await controller.exportMeetings(<({String meetingId, String title})>[
        (meetingId: 'm1', title: 'Sync'),
        (meetingId: 'm2', title: 'Sync'),
      ], MeetingExportFormat.json);

      expect(message, isNull);
      expect(sharer.shares.single.$1, <String>['Sync.json', 'Sync (2).json']);
      expect(sharer.shares.single.$2, '2 meetings');
    });

    test('failures are said', () async {
      final MeetingExportController controller = MeetingExportController(
          exportBridge: FakeExportBridge(
              defaultDirectory: root,
              failure:
                  const MeetingExportFailure(errorCode: 'meeting_not_found')),
          exportSaver: FakeExportSaver(savesBytes: false));

      expect(
          await controller.exportMeetings(<({String meetingId, String title})>[
            (meetingId: 'm1', title: 'Sync'),
          ], MeetingExportFormat.pdf),
          'Some meetings could not be exported.');
    });
  });

  group('desktop', () {
    test('the core writes where the owner chose', () async {
      final String chosen = '${root.path}/Weekly sync.pdf';
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final FakeExportSaver saver =
          FakeExportSaver(savesBytes: false, chosenPath: chosen);
      final MeetingExportController controller =
          MeetingExportController(exportBridge: bridge, exportSaver: saver);

      final String? message = await controller.exportMeeting(
          meetingId: 'm1',
          title: 'Weekly sync',
          format: MeetingExportFormat.pdf);

      expect(saver.suggestedNames, <String>['Weekly sync.pdf']);
      expect(bridge.requests.single.outputPath, chosen);
      expect(bridge.requests.single.includeAttachments, isTrue);
      expect(message, 'Exported to $chosen');
    });

    test('a cancelled dialog exports nothing', () async {
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final MeetingExportController controller = MeetingExportController(
          exportBridge: bridge,
          exportSaver: FakeExportSaver(savesBytes: false));

      final String? message = await controller.exportMeeting(
          meetingId: 'm1',
          title: 'Weekly sync',
          format: MeetingExportFormat.pdf);

      expect(message, isNull);
      expect(bridge.requests, isEmpty);
    });

    test('without a save dialog it exports to the default folder', () async {
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final MeetingExportController controller = MeetingExportController(
          exportBridge: bridge,
          exportSaver: FakeExportSaver(savesBytes: false, dialogFails: true));

      final String? message = await controller.exportMeeting(
          meetingId: 'm1',
          title: 'Weekly sync',
          format: MeetingExportFormat.json);

      expect(bridge.requests.single.outputPath, isNull);
      expect(message, 'Exported to ${root.path}/m1.json');
    });
  });

  group('mobile', () {
    test('the export goes to the system save dialog and the copy is deleted',
        () async {
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final FakeExportSaver saver = FakeExportSaver(savesBytes: true);
      final MeetingExportController controller =
          MeetingExportController(exportBridge: bridge, exportSaver: saver);

      final String? message = await controller.exportMeeting(
          meetingId: 'm1',
          title: 'Weekly sync',
          format: MeetingExportFormat.markdown);

      expect(bridge.requests.single.outputPath, isNull);
      expect(bridge.requests.single.includeAttachments, isFalse,
          reason: 'one file is saved, so Markdown cannot carry its folder');
      expect(saver.suggestedNames, <String>['Weekly sync.md']);
      expect(String.fromCharCodes(saver.savedBytes!), 'exported m1');
      expect(File('${root.path}/m1.md').existsSync(), isFalse);
      expect(message, 'Export saved');
    });

    test('PDF keeps its attachments', () async {
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final MeetingExportController controller = MeetingExportController(
          exportBridge: bridge, exportSaver: FakeExportSaver(savesBytes: true));

      await controller.exportMeeting(
          meetingId: 'm1',
          title: 'Weekly sync',
          format: MeetingExportFormat.pdf);

      expect(bridge.requests.single.includeAttachments, isTrue);
    });

    test('a cancelled dialog still deletes the private copy', () async {
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final MeetingExportController controller = MeetingExportController(
          exportBridge: bridge,
          exportSaver: FakeExportSaver(savesBytes: true, bytesSaved: false));

      final String? message = await controller.exportMeeting(
          meetingId: 'm1',
          title: 'Weekly sync',
          format: MeetingExportFormat.json);

      expect(message, isNull);
      expect(File('${root.path}/m1.json').existsSync(), isFalse);
    });
  });

  test('a failed export explains what went wrong', () async {
    final MeetingExportController controller = MeetingExportController(
      exportBridge: FakeExportBridge(
          defaultDirectory: root,
          failure: const MeetingExportFailure(errorCode: 'meeting_not_found')),
      exportSaver: FakeExportSaver(savesBytes: true),
    );

    final String? message = await controller.exportMeeting(
        meetingId: 'm1', title: 'Weekly sync', format: MeetingExportFormat.pdf);

    expect(message, 'This meeting no longer exists');
  });

  group('share', () {
    late Directory shareFolder;
    setUp(() => shareFolder = Directory('${root.path}/shared/handoff'));

    test('Markdown is shared with its attachments, flattened', () async {
      shareFolder.createSync(recursive: true);
      File('${shareFolder.path}/old.md').writeAsStringSync('earlier share');
      final FakeExportBridge bridge = FakeExportBridge(
          defaultDirectory: root,
          attachmentNames: <String>['board.jpg', 'Weekly sync.md']);
      final FakeExportSharer sharer = FakeExportSharer(shareFolder);
      final MeetingExportController controller = MeetingExportController(
          exportBridge: bridge,
          exportSaver: FakeExportSaver(savesBytes: true),
          exportSharer: sharer);
      expect(controller.canShare, isTrue);

      final String? message = await controller.shareMeeting(
          meetingId: 'm1',
          title: 'Weekly sync',
          format: MeetingExportFormat.markdown);

      expect(message, isNull);
      expect(bridge.requests.single.outputPath,
          '${shareFolder.path}/Weekly sync.md');
      expect(bridge.requests.single.includeAttachments, isTrue);
      expect(sharer.shares.single.$1, <String>[
        'Weekly sync.md',
        'Weekly sync (2).md',
        'board.jpg',
      ]);
      expect(sharer.shares.single.$2, 'Weekly sync');
      expect(File('${shareFolder.path}/old.md').existsSync(), isFalse,
          reason: 'an earlier share is cleared first');
      expect(
          Directory('${shareFolder.path}/Weekly sync_attachments').existsSync(),
          isFalse);
    });

    test('a failed export or share sheet is worded', () async {
      final MeetingExportController failing = MeetingExportController(
          exportBridge: FakeExportBridge(
              defaultDirectory: root,
              failure:
                  const MeetingExportFailure(errorCode: 'meeting_not_found')),
          exportSaver: FakeExportSaver(savesBytes: true),
          exportSharer: FakeExportSharer(shareFolder));
      expect(
          await failing.shareMeeting(
              meetingId: 'm1', title: 'A', format: MeetingExportFormat.pdf),
          'This meeting no longer exists');

      final MeetingExportController noSheet = MeetingExportController(
          exportBridge: FakeExportBridge(defaultDirectory: root),
          exportSaver: FakeExportSaver(savesBytes: true),
          exportSharer: FakeExportSharer(shareFolder, fails: true));
      expect(
          await noSheet.shareMeeting(
              meetingId: 'm1', title: 'A', format: MeetingExportFormat.pdf),
          'Unable to share the export');
    });

    test('without a sharer there is nothing to share with', () {
      expect(
          MeetingExportController(
                  exportBridge: FakeExportBridge(defaultDirectory: root),
                  exportSaver: FakeExportSaver(savesBytes: false))
              .canShare,
          isFalse);
    });

    testWidgets('the Export menu offers Share entries when it can share',
        (WidgetTester tester) async {
      final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
      final FakeExportSharer sharer = FakeExportSharer(shareFolder);
      final MeetingExportController exportController = MeetingExportController(
          exportBridge: bridge,
          exportSaver: FakeExportSaver(savesBytes: true),
          exportSharer: sharer);
      final MeetingListController meetingController = MeetingListController(
          meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
        'meeting-titled': titledMeetingDetail
      }));
      await tester.pumpWidget(MaterialApp(
        home: MeetingDetailScreen(
          controller: meetingController,
          meetingId: 'meeting-titled',
          exportController: exportController,
        ),
      ));
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('Export'));
      await tester.pumpAndSettle();
      expect(find.text('Share as Markdown'), findsOneWidget);
      await tester.tap(find.text('Share as PDF'));
      await tester.pump(const Duration(seconds: 1));
      // The share does real file I/O: give it real time, then frames, until it is done.
      for (int round = 0; round < 50 && exportController.isExporting; round++) {
        await tester.runAsync(
            () => Future<void>.delayed(const Duration(milliseconds: 20)));
        await tester.pump();
      }
      await tester.pumpAndSettle();

      expect(bridge.requests.single.format, MeetingExportFormat.pdf);
      expect(sharer.shares.single.$1, hasLength(1));
    });
  });

  testWidgets('the meeting offers Export with a format choice',
      (WidgetTester tester) async {
    final FakeExportBridge bridge = FakeExportBridge(defaultDirectory: root);
    final String chosen = '${root.path}/Project Sync.pdf';
    final MeetingExportController exportController = MeetingExportController(
        exportBridge: bridge,
        exportSaver: FakeExportSaver(savesBytes: false, chosenPath: chosen));
    final MeetingListController meetingController = MeetingListController(
        meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
      'meeting-titled': titledMeetingDetail
    }));
    await tester.pumpWidget(MaterialApp(
      home: MeetingDetailScreen(
        controller: meetingController,
        meetingId: 'meeting-titled',
        exportController: exportController,
      ),
    ));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Export'));
    await tester.pumpAndSettle();
    expect(find.text('Markdown'), findsOneWidget);
    expect(find.text('JSON'), findsOneWidget);
    await tester.tap(find.text('PDF'));
    await tester.pumpAndSettle();

    expect(bridge.requests.single.meetingId, 'meeting-titled');
    expect(bridge.requests.single.format, MeetingExportFormat.pdf);
    expect(find.text('Exported to $chosen'), findsOneWidget);
  });
}
