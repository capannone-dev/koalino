// Verifies exporting a meeting's summary as PDF, plain text, Word or Markdown (#130): the save
// dialog on desktops, the system picker on phones, the messages, and the Export menu on the
// summary page.

import 'dart:io';
import 'dart:typed_data';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/controllers/summary_export_controller.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_edit_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_summary_screen.dart';

import 'fake_llm_core.dart';

/// Writes a small file where asked, like the core, or fails with `failWith`.
final class FakeSummaryExportBridge implements SummaryExportBridge {
  /// The calls: meeting, format, path.
  final List<(String, SummaryExportFormat, String)> calls =
      <(String, SummaryExportFormat, String)>[];

  /// When set, every export fails with this code.
  String? failWith;

  @override
  Future<MeetingExportResult> exportSummary(
      String meetingId, SummaryExportFormat format, String outputPath) async {
    calls.add((meetingId, format, outputPath));
    if (failWith case final String code) {
      return MeetingExportFailure(errorCode: code);
    }
    await File(outputPath).writeAsBytes(<int>[1, 2, 3]);
    return MeetingExportSuccess(filePath: outputPath, sizeBytes: 3);
  }
}

/// A save dialog that answers with `chosenPath`, or takes bytes like a phone's picker.
final class FakeSaver implements ExportSaver {
  FakeSaver({this.chosenPath, this.savesBytes = false, this.saves = true});

  final String? chosenPath;
  @override
  final bool savesBytes;

  /// Whether the phone picker saves (false: the owner cancelled).
  final bool saves;

  final List<String> suggestedNames = <String>[];
  Uint8List? savedBytes;

  @override
  Future<String?> chooseSavePath(String suggestedFileName) async {
    suggestedNames.add(suggestedFileName);
    return chosenPath;
  }

  @override
  Future<bool> saveBytes(String suggestedFileName, Uint8List bytes) async {
    suggestedNames.add(suggestedFileName);
    savedBytes = bytes;
    return saves;
  }
}

void main() {
  late Directory folder;
  late FakeSummaryExportBridge bridge;

  setUp(() async {
    folder = await Directory.systemTemp.createTemp('summary-export-test');
    bridge = FakeSummaryExportBridge();
  });

  tearDown(() async {
    if (await folder.exists()) {
      await folder.delete(recursive: true);
    }
  });

  SummaryExportController controllerWith(FakeSaver saver) =>
      SummaryExportController(
          bridge: bridge,
          saver: saver,
          privateDirectory: () async => folder.path);

  test('a desktop saves where the owner chose, named after the meeting',
      () async {
    final String chosen = '${folder.path}/chosen.docx';
    final FakeSaver saver = FakeSaver(chosenPath: chosen);
    final String? message = await controllerWith(saver).exportSummary(
        meetingId: 'm-1',
        title: 'Budget: Q4/Q1',
        format: SummaryExportFormat.word);

    expect(saver.suggestedNames, <String>['Budget_ Q4_Q1 - Summary.docx']);
    expect(bridge.calls.single, ('m-1', SummaryExportFormat.word, chosen));
    expect(message, 'Exported to $chosen');
  });

  test('cancelling the save dialog exports nothing', () async {
    final String? message = await controllerWith(FakeSaver()).exportSummary(
        meetingId: 'm-1', title: 'Budget', format: SummaryExportFormat.pdf);
    expect(message, isNull);
    expect(bridge.calls, isEmpty);
  });

  test('a phone hands the bytes to the picker and keeps no copy', () async {
    final FakeSaver saver = FakeSaver(savesBytes: true);
    final String? message = await controllerWith(saver).exportSummary(
        meetingId: 'm-1', title: 'Budget', format: SummaryExportFormat.text);

    expect(message, 'Export saved');
    expect(saver.savedBytes, <int>[1, 2, 3]);
    expect(saver.suggestedNames, <String>['Budget - Summary.txt']);
    expect(folder.listSync(), isEmpty, reason: 'the private copy is deleted');
  });

  test('failures are explained', () async {
    final SummaryExportController controller =
        controllerWith(FakeSaver(chosenPath: '${folder.path}/s.pdf'));
    bridge.failWith = 'export_failed';
    expect(
        await controller.exportSummary(
            meetingId: 'm-1', title: 'B', format: SummaryExportFormat.pdf),
        'Unable to export the summary');
    bridge.failWith = 'no_summary';
    expect(
        await controller.exportSummary(
            meetingId: 'm-1', title: 'B', format: SummaryExportFormat.pdf),
        'No summary yet.');
  });

  test('the formats are the ones the core knows', () {
    expect(
        SummaryExportFormat.values
            .map((SummaryExportFormat format) => format.coreName),
        <String>['pdf', 'txt', 'docx', 'md']);
  });

  testWidgets('the summary page exports the summary in the chosen format',
      (WidgetTester tester) async {
    final FakeLlmCore core = FakeLlmCore();
    core.answer('command:settings', settings());
    core.answer('note_taker_get_meeting_summary', <String, Object?>{
      'ok': true,
      'summary': summary,
      'meeting_prompt': null,
    });
    final String chosen = '${folder.path}/summary.pdf';
    final SummariesController summaries = SummariesController(
      bridge: LlmBridge(call: core.call),
      editBridge: MeetingEditBridge(call: core.call),
      summaryExport: controllerWith(FakeSaver(chosenPath: chosen)),
    );
    await tester.pumpWidget(MaterialApp(
        home: MeetingSummaryScreen(
            controller: summaries, meetingId: 'm-1', title: 'Budget review')));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Export'));
    await tester.pumpAndSettle();
    for (final String label in <String>[
      'PDF',
      'Plain text (.txt)',
      'Word (.docx)',
      'Markdown (.md)'
    ]) {
      expect(find.text(label), findsOneWidget);
    }
    await tester.tap(find.text('PDF'));
    await tester.pump();
    // The fake bridge writes a real file; let that finish outside the fake clock.
    for (int attempt = 0;
        attempt < 50 && find.textContaining('Exported to').evaluate().isEmpty;
        attempt++) {
      await tester.runAsync(
          () => Future<void>.delayed(const Duration(milliseconds: 20)));
      await tester.pump();
    }
    await tester.pump(const Duration(milliseconds: 500));

    expect(bridge.calls.single, ('m-1', SummaryExportFormat.pdf, chosen));
    expect(find.text('Exported to $chosen'), findsOneWidget);
  });

  testWidgets('without a summary there is nothing to export',
      (WidgetTester tester) async {
    final FakeLlmCore core = FakeLlmCore();
    core.answer('command:settings', settings());
    core.answer('note_taker_get_meeting_summary',
        <String, Object?>{'ok': true, 'summary': null, 'meeting_prompt': null});
    final SummariesController summaries = SummariesController(
      bridge: LlmBridge(call: core.call),
      editBridge: MeetingEditBridge(call: core.call),
      summaryExport: controllerWith(FakeSaver()),
    );
    await tester.pumpWidget(MaterialApp(
        home: MeetingSummaryScreen(
            controller: summaries, meetingId: 'm-1', title: 'Budget review')));
    await tester.pumpAndSettle();
    expect(find.byTooltip('Export'), findsNothing);
  });
}
