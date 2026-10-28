// Defines the JSON contract for meeting attachments (Issue #54): attachments listed with a
// meeting, the attach-files response, and where each attachment sits in the transcript.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/meeting_attachment_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_timeline.dart';

/// A segment from `startTimeMs` to two seconds later.
MeetingTranscriptSegment segment(int startTimeMs, String text) =>
    MeetingTranscriptSegment(
        startTimeMs: startTimeMs,
        endTimeMs: startTimeMs + 2000,
        speaker: 'Alice',
        text: text);

/// An image attachment at `meetingTimeMs` (null: added after the meeting).
MeetingAttachment photo(String name, int? meetingTimeMs) => MeetingAttachment(
      attachmentId: 'id-$name',
      fileName: name,
      filePath: '/data/attachments/m/id-$name',
      mimeType: 'image/jpeg',
      meetingTimeMs: meetingTimeMs,
    );

/// Verifies attachment decoding and their placement among the transcript's segments.
void main() {
  test('a meeting lists its attachments, and older responses have none', () {
    final MeetingDetail meeting = decodeMeetingDetail('''
{"meeting_id":"m","title":"Sync","created_at":"2026-10-27T09:05:00+00:00","duration_ms":4000,
 "transcript":null,"segments":[],
 "attachments":[
   {"attachment_id":"a1","file_name":"board.jpg","file_path":"/data/a1-board.jpg",
    "mime_type":"image/jpeg","meeting_time_ms":2000},
   {"attachment_id":"a2","file_name":"agenda.pdf","file_path":"/data/a2-agenda.pdf",
    "mime_type":"application/pdf","meeting_time_ms":null}]}''');
    final MeetingDetail olderMeeting = decodeMeetingDetail('''
{"meeting_id":"m","title":"Sync","created_at":"2026-10-27T09:05:00+00:00","segments":[]}''');

    expect(meeting.attachments, hasLength(2));
    expect(meeting.attachments[0].fileName, 'board.jpg');
    expect(meeting.attachments[0].filePath, '/data/a1-board.jpg');
    expect(meeting.attachments[0].meetingTimeMs, 2000);
    expect(meeting.attachments[0].isImage, isTrue);
    expect(meeting.attachments[1].meetingTimeMs, isNull);
    expect(meeting.attachments[1].isImage, isFalse);
    expect(olderMeeting.attachments, isEmpty);
  });

  test('the attach-files response lists what was attached and what failed', () {
    final AttachFilesResult result = decodeAttachFilesResponse('''
{"ok":true,
 "attached":[{"attachment_id":"a1","file_name":"board.jpg","file_path":"/data/a1-board.jpg",
              "mime_type":"image/jpeg","meeting_time_ms":2000}],
 "failed":[{"path":"/tmp/tool.exe","error":"only pictures and documents can be attached"}]}''');

    expect(result.attached.single.fileName, 'board.jpg');
    expect(result.failedPaths, <String>['/tmp/tool.exe']);
  });

  test('a rejected attach request throws a StateError', () {
    expect(
      () => decodeAttachFilesResponse(
          '{"ok":false,"error_code":"meeting_not_found","error":"gone"}'),
      throwsA(isA<StateError>().having((StateError error) => error.message,
          'message', contains('meeting_not_found'))),
    );
    expect(() => decodeAttachFilesResponse('[]'), throwsFormatException);
  });

  test(
      'attachments sit after the segments that started before them, '
      'and the untimed ones at the end', () {
    final List<MeetingTimelineEntry> timeline = buildMeetingTimeline(
      segments: <MeetingTranscriptSegment>[
        segment(0, 'Hello.'),
        segment(5000, 'Next slide.'),
        segment(9000, 'Bye.'),
      ],
      attachments: <MeetingAttachment>[
        photo('after.jpg', null),
        photo('second.jpg', 5000),
        photo('first.jpg', 1000),
        photo('late.jpg', 60000),
      ],
    );

    expect(
      timeline
          .map((MeetingTimelineEntry entry) => switch (entry) {
                SegmentEntry(:final MeetingTranscriptSegment segment) =>
                  segment.text,
                AttachmentEntry(:final MeetingAttachment attachment) =>
                  attachment.fileName,
              })
          .toList(),
      <String>[
        'Hello.',
        'first.jpg',
        'Next slide.',
        'second.jpg',
        'Bye.',
        'late.jpg',
        'after.jpg',
      ],
    );
  });
}
