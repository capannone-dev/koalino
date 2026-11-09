// Specifies how the app shows sealed attachments (#99): the core's answer is read into a path or
// a stable error code, and a view waits for the clear copy, then shows it, or says it cannot.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/clear_copy_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/clear_file.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';

import 'fake_meeting_bridge.dart';

void main() {
  group('decodeClearCopyResponse', () {
    test('an answer with a path gives the path', () {
      expect(
          decodeClearCopyResponse(
              '{"ok":true,"path":"/data/clear-copies/a1-board.jpg"}'),
          '/data/clear-copies/a1-board.jpg');
    });

    test('a refusal carries the core code', () {
      expect(
          () => decodeClearCopyResponse(
              '{"ok":false,"error_code":"not_found","error":"…"}'),
          throwsA(isA<ClearCopyException>().having(
              (ClearCopyException error) => error.code, 'code', 'not_found')));
    });

    test('anything else is a bad response', () {
      for (final String answer in <String>['[]', '{}', '{"ok":true}']) {
        expect(
            () => decodeClearCopyResponse(answer),
            throwsA(isA<ClearCopyException>().having(
                (ClearCopyException error) => error.code,
                'code',
                'bad_response')));
      }
    });
  });

  group('ClearFile', () {
    late ClearCopyResolver original;
    setUp(() => original = clearCopyResolver);
    tearDown(() => clearCopyResolver = original);

    Widget view() => MaterialApp(
          home: Scaffold(
            body: ClearFile(
              attachmentId: 'a1',
              builder: (BuildContext context, String path) => Text('at $path'),
              unavailableBuilder: (BuildContext context) =>
                  const Text('cannot be opened'),
            ),
          ),
        );

    testWidgets('shows the view built from the copy',
        (WidgetTester tester) async {
      final List<String> asked = <String>[];
      clearCopyResolver = (String attachmentId) async {
        asked.add(attachmentId);
        return '/copies/$attachmentId-board.jpg';
      };

      await tester.pumpWidget(view());
      await tester.pumpAndSettle();

      expect(find.text('at /copies/a1-board.jpg'), findsOneWidget);
      expect(asked, <String>['a1']);
    });

    testWidgets('says when there is no copy', (WidgetTester tester) async {
      clearCopyResolver = (String attachmentId) async =>
          throw const ClearCopyException('unreadable');

      await tester.pumpWidget(view());
      await tester.pumpAndSettle();

      expect(find.text('cannot be opened'), findsOneWidget);
    });
  });

  testWidgets('a project PDF opens from its clear copy, never the sealed file',
      (WidgetTester tester) async {
    final ProjectResource pdf = ProjectResource(
      attachmentId: 'r1',
      fileName: 'plan.pdf',
      mimeType: 'application/pdf',
      kind: ProjectResourceKind.pdf,
      addedAt: DateTime.utc(2026, 11, 9),
      sizeBytes: 2048,
      filePath: '/data/attachments/project-x/r1-plan.pdf',
    );
    await tester.pumpWidget(MaterialApp(
      home: ResourceViewerScreen(
        controller: ProjectsController(
          projectBridge: ProjectBridge(
              runCommand: (String request) async =>
                  '{"ok":false,"error_code":"invalid_request"}'),
          meetingBridge: FakeMeetingBridge(),
          filesPicker: const _NoPicker(),
        ),
        projectId: 'project-x',
        resource: pdf,
        pdfViewBuilder: (String filePath) => Text('PDF $filePath'),
      ),
    ));
    await tester.pumpAndSettle();

    expect(find.text('PDF /clear-copies/r1'), findsOneWidget);
  });
}

/// A picker never opened here.
final class _NoPicker implements ProjectFilesPicker {
  const _NoPicker();

  @override
  Future<List<String>> pickFiles() async => const <String>[];
}
