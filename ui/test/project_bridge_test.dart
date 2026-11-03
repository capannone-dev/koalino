// Specifies the Dart side of `note_taker_project_command` (Issue #85): requests carry the right
// command and fields, answers decode into models, and refusals become exceptions.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';

import 'fake_project_runner.dart';

void main() {
  test('a project detail decodes its meetings and resources', () async {
    final FakeProjectRunner runner =
        FakeProjectRunner((Map<String, Object?> request) => <String, Object?>{
              'ok': true,
              'project': projectJson('p1', 'Hiring', meetingCount: 1),
              'meetings': <Object?>[
                <String, Object?>{
                  'meeting_id': 'meeting-1',
                  'title': 'Kick-off',
                  'created_at': '2026-11-02T09:00:00+00:00',
                  'duration_ms': 60000,
                  'project_id': 'p1',
                },
              ],
              'resources': <Object?>[
                resourceJson('r1', 'plan.md', 'markdown',
                    mimeType: 'text/markdown'),
                resourceJson('r2', 'offer.eml', 'email', sizeBytes: null),
                resourceJson('r3', 'odd.bin', 'hologram'),
              ],
            });

    final ProjectDetail detail =
        await ProjectBridge(runCommand: runner.call).projectDetail('p1');

    expect(runner.requests.single,
        <String, Object?>{'command': 'detail', 'project_id': 'p1'});
    expect(detail.project.name, 'Hiring');
    expect(detail.meetings.single.title, 'Kick-off');
    expect(detail.resources.map((ProjectResource r) => r.kind), <Object>[
      ProjectResourceKind.markdown,
      ProjectResourceKind.email,
      ProjectResourceKind.document,
    ]);
    expect(detail.resources[1].isAvailable, isFalse);
  });

  test('moving a meeting out sends a null project', () async {
    final FakeProjectRunner runner = FakeProjectRunner(
        (Map<String, Object?> request) => <String, Object?>{'ok': true});

    await ProjectBridge(runCommand: runner.call).moveMeeting('meeting-1', null);

    expect(runner.requests.single, <String, Object?>{
      'command': 'move_meeting',
      'meeting_id': 'meeting-1',
      'project_id': null,
    });
  });

  test('a refusal carries its error code', () async {
    final FakeProjectRunner runner =
        FakeProjectRunner((Map<String, Object?> request) => <String, Object?>{
              'ok': false,
              'error_code': 'invalid_name',
              'error': 'blank',
            });

    await expectLater(
      ProjectBridge(runCommand: runner.call).createProject(' '),
      throwsA(isA<ProjectCommandException>().having(
          (ProjectCommandException error) => error.errorCode,
          'errorCode',
          'invalid_name')),
    );
  });

  test('an e-mail decodes its people, time and attachment names', () {
    final EmailView email = decodeEmailView(<String, Object?>{
      'subject': 'Offer',
      'from': <Object?>[
        <String, Object?>{'name': 'Anna', 'address': 'anna@example.com'},
      ],
      'to': <Object?>[
        <String, Object?>{'name': null, 'address': 'ettore@example.com'},
      ],
      'cc': <Object?>[],
      'sent_at': '2026-11-03T09:15:00+00:00',
      'body_text': 'Yes!',
      'body_truncated': false,
      'attachments': <Object?>[
        <String, Object?>{
          'file_name': 'cv.pdf',
          'mime_type': 'application/pdf',
          'size_bytes': 9,
        },
      ],
    });

    expect(email.from.single.label, 'Anna <anna@example.com>');
    expect(email.to.single.label, 'ettore@example.com');
    expect(email.sentAt, DateTime.utc(2026, 11, 3, 9, 15));
    expect(email.attachmentNames, <String>['cv.pdf']);
  });

  test('adding files reports what failed and what was left in e-mails',
      () async {
    final FakeProjectRunner runner =
        FakeProjectRunner((Map<String, Object?> request) => <String, Object?>{
              'ok': true,
              'added': <Object?>[resourceJson('r1', 'a.png', 'image')],
              'failed': <Object?>[
                <String, Object?>{'path': '/tmp/setup.exe', 'error': 'no'},
              ],
              'skipped_attachments': 2,
            });

    final AddProjectFilesResult result =
        await ProjectBridge(runCommand: runner.call)
            .addFiles('p1', <String>['/tmp/a.png', '/tmp/setup.exe']);

    expect(runner.requests.single['paths'],
        <String>['/tmp/a.png', '/tmp/setup.exe']);
    expect(result.added.single.fileName, 'a.png');
    expect(result.failedPaths, <String>['/tmp/setup.exe']);
    expect(result.skippedAttachments, 2);
  });
}
