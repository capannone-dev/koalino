// Specifies the Projects tab (Issue #85): listing and creating projects, a project's page with
// its meetings and resources, the e-mail and Markdown viewers, deleting a resource, moving a
// meeting from its page, and the messages after adding files.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/markdown_view.dart';
import 'package:note_taker_ui/presentation/project_dialogs.dart';
import 'package:note_taker_ui/presentation/projects_screen.dart';

import 'fake_meeting_bridge.dart';
import 'fake_project_runner.dart';

/// An in-memory stand-in for Rust's projects, answering the commands the tab sends.
final class FakeProjects {
  /// Project names by ID.
  final Map<String, String> names = <String, String>{'p1': 'Hiring'};

  /// Resources by project ID.
  final Map<String, List<Map<String, Object?>>> resources =
      <String, List<Map<String, Object?>>>{
    'p1': <Map<String, Object?>>[
      resourceJson('r-mail', 'offer.eml', 'email', mimeType: 'message/rfc822'),
      resourceJson('r-doc', 'budget.xlsx', 'document'),
    ],
  };

  /// Project of each meeting.
  final Map<String, String> memberships = <String, String>{
    'meeting-1': 'p1',
  };

  late final FakeProjectRunner runner = FakeProjectRunner(_answer);

  Map<String, Object?> _answer(Map<String, Object?> request) {
    final String? projectId = request['project_id'] as String?;
    return switch (request['command']) {
      'list' => <String, Object?>{
          'ok': true,
          'projects': <Object?>[
            for (final MapEntry<String, String> entry in names.entries)
              projectJson(entry.key, entry.value,
                  resourceCount: resources[entry.key]?.length ?? 0),
          ],
        },
      'create' => () {
          names['p2'] = request['name']! as String;
          return <String, Object?>{
            'ok': true,
            'project': projectJson('p2', names['p2']!),
          };
        }(),
      'detail' => <String, Object?>{
          'ok': true,
          'project': projectJson(projectId!, names[projectId]!),
          'meetings': <Object?>[
            for (final MapEntry<String, String> entry in memberships.entries)
              if (entry.value == projectId)
                <String, Object?>{
                  'meeting_id': entry.key,
                  'title': 'Kick-off',
                  'created_at': '2026-11-02T09:00:00+00:00',
                  'duration_ms': 0,
                  'project_id': projectId,
                },
          ],
          'resources': resources[projectId] ?? <Object?>[],
        },
      'delete_resource' => () {
          resources[projectId]!.removeWhere((Map<String, Object?> resource) =>
              resource['attachment_id'] == request['attachment_id']);
          return <String, Object?>{'ok': true, 'deleted': true};
        }(),
      'read_email' => <String, Object?>{
          'ok': true,
          'email': <String, Object?>{
            'subject': 'Your offer',
            'from': <Object?>[
              <String, Object?>{'name': 'Anna', 'address': 'anna@example.com'},
            ],
            'to': <Object?>[],
            'cc': <Object?>[],
            'sent_at': null,
            'body_text': 'I accept.',
            'body_truncated': false,
            'attachments': <Object?>[],
          },
        },
      'meeting_projects' => <String, Object?>{
          'ok': true,
          'meeting_projects': memberships,
        },
      'move_meeting' => () {
          if (projectId == null) {
            memberships.remove(request['meeting_id']);
          } else {
            memberships[request['meeting_id']! as String] = projectId;
          }
          return <String, Object?>{'ok': true};
        }(),
      _ => <String, Object?>{'ok': false, 'error_code': 'invalid_request'},
    };
  }

  /// A controller over this fake.
  ProjectsController controller(
          {List<String> pickedFiles = const <String>[]}) =>
      ProjectsController(
        projectBridge: ProjectBridge(runCommand: runner.call),
        meetingBridge: FakeMeetingBridge(meetings: <MeetingSummary>[
          MeetingSummary(
            meetingId: 'meeting-2',
            title: 'Budget review',
            createdAt: DateTime.utc(2026, 11, 1),
            durationMs: 0,
          ),
        ]),
        filesPicker: _FixedPicker(pickedFiles),
      );
}

/// A picker that returns fixed paths.
final class _FixedPicker implements ProjectFilesPicker {
  const _FixedPicker(this.paths);

  final List<String> paths;

  @override
  Future<List<String>> pickFiles() async => paths;
}

/// Pumps the tab at phone size.
Future<void> _pumpTab(
    WidgetTester tester, ProjectsController controller) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(390, 844);
  addTearDown(tester.view.reset);
  await tester.pumpWidget(MaterialApp(
    home: ProjectsScreen(
      controller: controller,
      pdfViewBuilder: (String filePath) => Text('PDF $filePath'),
    ),
  ));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('projects are listed and a new one opens after naming it',
      (WidgetTester tester) async {
    final FakeProjects projects = FakeProjects();
    await _pumpTab(tester, projects.controller());

    expect(find.text('Hiring'), findsOneWidget);
    expect(find.text('0 meetings · 2 files'), findsOneWidget);

    await tester.tap(find.byKey(const ValueKey<String>('new-project')));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Create'));
    await tester.pumpAndSettle();
    expect(find.text('Enter a name.'), findsOneWidget);

    await tester.enterText(
        find.byKey(const ValueKey<String>('project-name-field')), ' Budget ');
    await tester.tap(find.text('Create'));
    await tester.pumpAndSettle();

    expect(
        projects.runner.requests.firstWhere(
            (Map<String, Object?> r) => r['command'] == 'create')['name'],
        'Budget');
    expect(find.widgetWithText(AppBar, 'Budget'), findsOneWidget);
  });

  testWidgets(
      'a project shows its meetings and resources, reads e-mails and deletes files',
      (WidgetTester tester) async {
    final FakeProjects projects = FakeProjects();
    await _pumpTab(tester, projects.controller());

    await tester.tap(find.text('Hiring'));
    await tester.pumpAndSettle();
    expect(find.text('Kick-off'), findsOneWidget);
    expect(find.text('offer.eml'), findsOneWidget);
    expect(find.text('budget.xlsx'), findsOneWidget);

    await tester.tap(find.text('offer.eml'));
    await tester.pumpAndSettle();
    expect(find.text('Your offer'), findsOneWidget);
    expect(find.text('I accept.'), findsOneWidget);
    await tester.pageBack();
    await tester.pumpAndSettle();

    await tester.tap(find.text('budget.xlsx'));
    await tester.pumpAndSettle();
    expect(find.textContaining("can't be shown in the app"), findsOneWidget);
    await tester.pageBack();
    await tester.pumpAndSettle();

    await tester.tap(find.descendant(
      of: find.byKey(const ValueKey<String>('project-resource-r-doc')),
      matching: find.byTooltip('Delete'),
    ));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Delete'));
    await tester.pumpAndSettle();

    expect(projects.runner.commands, contains('delete_resource'));
    expect(find.text('budget.xlsx'), findsNothing);
  });

  testWidgets('a meeting moves into a project from its page',
      (WidgetTester tester) async {
    final FakeProjects projects = FakeProjects();
    projects.memberships.clear();
    final ProjectsController controller = projects.controller();
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        appBar: AppBar(actions: <Widget>[
          MoveToProjectButton(controller: controller, meetingId: 'meeting-1'),
        ]),
      ),
    ));

    await tester.tap(find.byKey(const ValueKey<String>('move-to-project')));
    await tester.pumpAndSettle();
    expect(find.text('No project'), findsOneWidget);
    await tester.tap(find.text('Hiring'));
    await tester.pumpAndSettle();

    expect(projects.memberships, <String, String>{'meeting-1': 'p1'});
    expect(find.text('Moved to Hiring'), findsOneWidget);
  });

  test('adding files explains failures and attachments left in e-mails',
      () async {
    final FakeProjectRunner runner = FakeProjectRunner(
        (Map<String, Object?> request) => switch (request['command']) {
              'add_files' => <String, Object?>{
                  'ok': true,
                  'added': <Object?>[
                    resourceJson('r1', 'offer.eml', 'email'),
                    resourceJson('r2', 'cv.pdf', 'pdf'),
                  ],
                  'failed': <Object?>[
                    <String, Object?>{'path': '/x/setup.exe', 'error': 'no'},
                  ],
                  'skipped_attachments': 1,
                },
              'list' => <String, Object?>{'ok': true, 'projects': <Object?>[]},
              _ => <String, Object?>{'ok': true},
            });
    final ProjectsController controller = ProjectsController(
      projectBridge: ProjectBridge(runCommand: runner.call),
      meetingBridge: FakeMeetingBridge(),
      filesPicker: const _FixedPicker(<String>['/x/offer.eml', '/x/setup.exe']),
    );

    expect(await controller.addFiles('p1'),
        'Added 2 files; 1 could not be added; 1 e-mail attachment was left in the e-mail');
  });

  testWidgets('Markdown shows headings, lists and inert links',
      (WidgetTester tester) async {
    await tester.pumpWidget(const MaterialApp(
      home: Scaffold(
        body: MarkdownView(
          markdownText:
              '# Plan\n\n- call **Anna**\n- [site](https://example.com)\n\n'
              '![logo](https://example.com/logo.png)',
        ),
      ),
    ));

    expect(find.text('Plan', findRichText: true), findsOneWidget);
    expect(find.text('call Anna', findRichText: true), findsOneWidget);
    expect(find.text('site', findRichText: true), findsOneWidget);
    expect(find.text('[logo]', findRichText: true), findsOneWidget);
    expect(find.byType(Image), findsNothing);
  });
}
