// Defines widget-first requirements for attachment file selection and meeting linking.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/attachment_controller.dart';
import 'package:note_taker_ui/infrastructure/attachment_bridge.dart';
import 'package:note_taker_ui/presentation/attachment_import_screen.dart';

/// Simulates a platform picker without opening native picker UI in widget tests.
final class FakeAttachmentFilePicker implements AttachmentFilePicker {
  /// Creates a picker that returns `selectedPath` when the import control is used.
  FakeAttachmentFilePicker(this.selectedPath);

  /// Controls the native path supplied to the controller.
  final String? selectedPath;

  /// Counts user-initiated picker requests.
  int pickCallCount = 0;

  @override
  Future<String?> pickAttachmentPath() async {
    pickCallCount += 1;
    return selectedPath;
  }
}

/// Records raw JSON FFI interactions without loading the Rust shared library.
final class FakeAttachmentBridge implements AttachmentBridge {
  /// Creates a bridge returning `suggestionsJson` for every selected local path.
  FakeAttachmentBridge(this.suggestionsJson);

  /// Raw response supplied by the Rust FFI contract.
  final String suggestionsJson;

  /// Paths submitted to Rust for metadata-based suggestion lookup.
  final List<String> processedPaths = <String>[];

  /// Attachment and meeting pairs approved by the user.
  final List<(String, String)> confirmedLinks = <(String, String)>[];

  @override
  Future<void> confirmAttachmentLink({
    required String attachmentId,
    required String meetingId,
  }) async {
    confirmedLinks.add((attachmentId, meetingId));
  }

  @override
  Future<String> processAttachment(String filePath) async {
    processedPaths.add(filePath);
    return suggestionsJson;
  }
}

/// Verifies attachment-import behavior through picker and FFI interfaces.
void main() {
  const String suggestionsJson = '''[
    {
      "attachment_id": "attachment-1",
      "meeting_id": "meeting-team-a",
      "meeting_start_time": "2026-10-25T09:00:00Z",
      "meeting_end_time": "2026-10-25T10:00:00Z",
      "confidence_score": 0.95
    },
    {
      "attachment_id": "attachment-1",
      "meeting_id": "meeting-team-b",
      "meeting_start_time": "2026-10-25T11:00:00Z",
      "meeting_end_time": "2026-10-25T12:00:00Z",
      "confidence_score": 0.7
    }
  ]''';

  /// Builds the Material host used by each widget contract test.
  Widget buildAttachmentImportApp({
    required AttachmentController attachmentController,
  }) {
    return MaterialApp(
      home: AttachmentImportScreen(attachmentController: attachmentController),
    );
  }

  testWidgets('Add Attachment triggers the picker and displays ranked suggestions',
      (WidgetTester tester) async {
    final FakeAttachmentFilePicker attachmentFilePicker =
        FakeAttachmentFilePicker('/private/agenda.pdf');
    final FakeAttachmentBridge attachmentBridge =
        FakeAttachmentBridge(suggestionsJson);
    final AttachmentController attachmentController = AttachmentController(
      attachmentBridge: attachmentBridge,
      attachmentFilePicker: attachmentFilePicker,
    );

    await tester.pumpWidget(
      buildAttachmentImportApp(attachmentController: attachmentController),
    );

    await tester.tap(find.byTooltip('Add Attachment'));
    await tester.pumpAndSettle();

    expect(attachmentFilePicker.pickCallCount, 1);
    expect(attachmentBridge.processedPaths, <String>['/private/agenda.pdf']);
    expect(find.text('Suggested meetings'), findsOneWidget);
    expect(find.text('meeting-team-a (95% match)'), findsOneWidget);
    expect(find.text('meeting-team-b (70% match)'), findsOneWidget);
    expect(find.text('Choose Different Meeting'), findsOneWidget);
    expect(find.text("Don't Link"), findsOneWidget);
  });

  testWidgets('selecting a suggestion confirms its attachment and meeting IDs',
      (WidgetTester tester) async {
    final FakeAttachmentBridge attachmentBridge =
        FakeAttachmentBridge(suggestionsJson);
    final AttachmentController attachmentController = AttachmentController(
      attachmentBridge: attachmentBridge,
      attachmentFilePicker: FakeAttachmentFilePicker('/private/agenda.pdf'),
    );

    await tester.pumpWidget(
      buildAttachmentImportApp(attachmentController: attachmentController),
    );
    await tester.tap(find.byTooltip('Add Attachment'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Select').first);
    await tester.pumpAndSettle();

    expect(attachmentBridge.confirmedLinks, <(String, String)>[
      ('attachment-1', 'meeting-team-a'),
    ]);
  });
}
