// Specifies shares from other apps (Issue #85): the sheet that picks a project for a share, a
// new project created from it, a dismissed sheet, a share that arrives while the app runs, and
// that the platform's copies are released every time.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/incoming_share_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/share_to_project.dart';

import 'fake_meeting_bridge.dart';
import 'fake_project_runner.dart';

/// Shares handed out in order, with the folders released.
final class _FakeShareBridge implements IncomingShareBridge {
  final List<IncomingShare> pending = <IncomingShare>[];
  final List<String> released = <String>[];
  final StreamController<void> arrivalEvents =
      StreamController<void>.broadcast();

  @override
  Stream<void> get arrivals => arrivalEvents.stream;

  @override
  Future<IncomingShare?> takePending() async =>
      pending.isEmpty ? null : pending.removeAt(0);

  @override
  Future<void> release(IncomingShare share) async =>
      released.add(share.directory);
}

/// A picker that is never used here.
final class _NoPicker implements ProjectFilesPicker {
  const _NoPicker();

  @override
  Future<List<String>> pickFiles() async => const <String>[];
}

/// One project, "Hiring"; `add_files` adds every path.
FakeProjectRunner _runner() => FakeProjectRunner(
      (Map<String, Object?> request) => switch (request['command']) {
        'list' => <String, Object?>{
            'ok': true,
            'projects': <Object?>[projectJson('p1', 'Hiring')],
          },
        'create' => <String, Object?>{
            'ok': true,
            'project': projectJson('p2', request['name']! as String),
          },
        'add_files' => <String, Object?>{
            'ok': true,
            'added': <Object?>[
              for (final Object? path in request['paths']! as List<Object?>)
                resourceJson('r-$path', '$path', 'document'),
            ],
            'failed': <Object?>[],
            'skipped_attachments': 0,
          },
        _ => <String, Object?>{'ok': true},
      },
    );

/// Pumps the listener around an empty page.
Future<void> _pump(WidgetTester tester, _FakeShareBridge bridge,
    FakeProjectRunner runner) async {
  final ProjectsController controller = ProjectsController(
    projectBridge: ProjectBridge(runCommand: runner.call),
    meetingBridge: FakeMeetingBridge(meetings: const <MeetingSummary>[]),
    filesPicker: const _NoPicker(),
  );
  await tester.pumpWidget(MaterialApp(
    home: IncomingShareListener(
      shareBridge: bridge,
      projectsController: controller,
      child: const Scaffold(body: Text('Home')),
    ),
  ));
  await tester.pumpAndSettle();
}

const IncomingShare _offerShare = IncomingShare(
  directory: '/cache/incoming/share-1',
  filePaths: <String>['/cache/incoming/share-1/offer.eml'],
  failedCount: 0,
);

void main() {
  testWidgets('a share pending at start-up goes into the chosen project',
      (WidgetTester tester) async {
    final _FakeShareBridge bridge = _FakeShareBridge()
      ..pending.add(_offerShare);
    final FakeProjectRunner runner = _runner();

    await _pump(tester, bridge, runner);

    expect(find.text('Add the shared file to a project'), findsOneWidget);
    await tester.tap(find.text('Hiring'));
    await tester.pumpAndSettle();

    final Map<String, Object?> addRequest = runner.requests.lastWhere(
        (Map<String, Object?> request) => request['command'] == 'add_files');
    expect(addRequest['project_id'], 'p1');
    expect(addRequest['paths'], _offerShare.filePaths);
    expect(find.text('Added 1 file'), findsOneWidget);
    expect(bridge.released, <String>[_offerShare.directory]);
  });

  testWidgets('New project creates one and adds the files to it',
      (WidgetTester tester) async {
    final _FakeShareBridge bridge = _FakeShareBridge()
      ..pending.add(const IncomingShare(
        directory: '/cache/incoming/share-2',
        filePaths: <String>['/a/cv.pdf', '/a/photo.jpg'],
        failedCount: 1,
      ));
    final FakeProjectRunner runner = _runner();

    await _pump(tester, bridge, runner);
    expect(find.text('Add 2 shared files to a project'), findsOneWidget);
    await tester.tap(find.text('New project'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'Offers');
    await tester.tap(find.text('Create'));
    await tester.pumpAndSettle();

    expect(runner.commands, contains('create'));
    final Map<String, Object?> addRequest = runner.requests.lastWhere(
        (Map<String, Object?> request) => request['command'] == 'add_files');
    expect(addRequest['project_id'], 'p2');
    expect(find.text('Added 2 files; 1 could not be read'), findsOneWidget);
    expect(bridge.released, <String>['/cache/incoming/share-2']);
  });

  testWidgets('dismissing the sheet adds nothing but releases the copies',
      (WidgetTester tester) async {
    final _FakeShareBridge bridge = _FakeShareBridge()
      ..pending.add(_offerShare);
    final FakeProjectRunner runner = _runner();

    await _pump(tester, bridge, runner);
    await tester.tapAt(const Offset(10, 10));
    await tester.pumpAndSettle();

    expect(runner.commands, isNot(contains('add_files')));
    expect(bridge.released, <String>[_offerShare.directory]);
  });

  testWidgets('a share arriving while the app runs opens the sheet',
      (WidgetTester tester) async {
    final _FakeShareBridge bridge = _FakeShareBridge();
    await _pump(tester, bridge, _runner());
    expect(find.text('Add the shared file to a project'), findsNothing);

    bridge.pending.add(_offerShare);
    bridge.arrivalEvents.add(null);
    await tester.pumpAndSettle();

    expect(find.text('Add the shared file to a project'), findsOneWidget);
  });

  testWidgets('a share with nothing readable says so',
      (WidgetTester tester) async {
    final _FakeShareBridge bridge = _FakeShareBridge()
      ..pending.add(const IncomingShare(
          directory: '/cache/incoming/share-3',
          filePaths: <String>[],
          failedCount: 2));

    await _pump(tester, bridge, _runner());

    expect(find.text('Nothing that was shared could be read.'), findsOneWidget);
    expect(bridge.released, <String>['/cache/incoming/share-3']);
  });

  test('decodes the channel map and refuses a malformed one', () {
    final IncomingShare? share = IncomingShare.fromChannel(<Object?, Object?>{
      'directory': '/d',
      'paths': <Object?>['/d/a.pdf'],
      'failedCount': 1,
    });
    expect(share?.filePaths, <String>['/d/a.pdf']);
    expect(share?.failedCount, 1);
    expect(IncomingShare.fromChannel(<Object?, Object?>{'paths': 3}), isNull);
    expect(IncomingShare.fromChannel(null), isNull);
  });
}
