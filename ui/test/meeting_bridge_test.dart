// Defines the JSON contract between the Rust meeting FFI calls and the Dart meeting models.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

/// Verifies `note_taker_get_meetings` and `note_taker_get_meeting` responses decode with titles.
void main() {
  group('meeting list decoding', () {
    test('parses titles, creation times, and durations', () {
      final List<MeetingSummary> meetings = decodeMeetings('''
[
  {"meeting_id":"meeting-1","title":"Project Sync","created_at":"2026-10-27T09:05:00+00:00","duration_ms":65000},
  {"meeting_id":"meeting-2","title":"Meeting 2026-10-26 18:00 UTC","created_at":"2026-10-26T18:00:00+00:00","duration_ms":0}
]''');

      expect(meetings, hasLength(2));
      expect(meetings[0].meetingId, 'meeting-1');
      expect(meetings[0].title, 'Project Sync');
      expect(meetings[0].createdAt, DateTime.utc(2026, 10, 27, 9, 5));
      expect(meetings[0].durationMs, 65000);
      expect(meetings[1].title, 'Meeting 2026-10-26 18:00 UTC');
    });

    test('falls back to a dated title when the title is null or blank', () {
      final List<MeetingSummary> meetings = decodeMeetings('''
[
  {"meeting_id":"meeting-null","title":null,"created_at":"2026-01-15T14:30:00+00:00"},
  {"meeting_id":"meeting-blank","title":"  ","created_at":"2026-01-15T07:03:00+00:00"}
]''');

      expect(meetings[0].title, 'Meeting 2026-01-15 14:30 UTC');
      expect(meetings[1].title, 'Meeting 2026-01-15 07:03 UTC');
      expect(meetings[0].durationMs, 0);
    });

    test('parses the transcription languages, and none when missing', () {
      final List<MeetingSummary> meetings = decodeMeetings('''
[
  {"meeting_id":"meeting-1","created_at":"2026-10-27T09:05:00+00:00","transcription_languages":["it","auto"]},
  {"meeting_id":"meeting-2","created_at":"2026-10-26T18:00:00+00:00"}
]''');

      expect(meetings[0].transcriptionLanguages, <String>['it', 'auto']);
      expect(meetings[1].transcriptionLanguages, isEmpty);
    });

    test('turns a Rust error object into a StateError', () {
      expect(
        () => decodeMeetings(
            '{"ok":false,"error_code":"list_failed","error":"disk"}'),
        throwsA(isA<StateError>().having((StateError error) => error.message,
            'message', contains('list_failed'))),
      );
    });

    test('rejects a response that is neither an array nor an error', () {
      expect(() => decodeMeetings('"meetings"'), throwsFormatException);
    });
  });

  group('meeting detail decoding', () {
    test('parses the title, segments, and a null flat transcript', () {
      final MeetingDetail meeting = decodeMeetingDetail('''
{"meeting_id":"meeting-1","title":"Project Sync","created_at":"2026-10-27T09:05:00+00:00",
 "duration_ms":4000,"transcript":null,
 "segments":[{"start_time_ms":0,"end_time_ms":1500,"speaker":"Alice","text":"Hello."},
             {"start_time_ms":1500,"end_time_ms":4000,"speaker":"Unattributed","text":"Hi."}]}''');

      expect(meeting.summary.title, 'Project Sync');
      expect(meeting.summary.durationMs, 4000);
      expect(meeting.transcript, isNull);
      expect(meeting.segments, hasLength(2));
      expect(meeting.segments[0].speaker, 'Alice');
      expect(meeting.segments[1].startTimeMs, 1500);
      expect(meeting.segments[1].text, 'Hi.');
    });

    test('accepts responses without segments and uses the fallback title', () {
      final MeetingDetail meeting = decodeMeetingDetail(
          '{"meeting_id":"m","created_at":"2026-01-15T14:30:00Z","transcript":"Flat."}');

      expect(meeting.summary.title, 'Meeting 2026-01-15 14:30 UTC');
      expect(meeting.transcript, 'Flat.');
      expect(meeting.segments, isEmpty);
    });

    test('turns meeting_not_found into a StateError', () {
      expect(
        () => decodeMeetingDetail(
            '{"ok":false,"error_code":"meeting_not_found","error":"missing"}'),
        throwsA(isA<StateError>().having((StateError error) => error.message,
            'message', contains('meeting_not_found'))),
      );
    });
  });

  group('rename status mapping', () {
    test('maps each Rust status code to a rename failure', () {
      expect(meetingRenameExceptionFor(1).failure,
          MeetingRenameFailure.invalidTitle);
      expect(meetingRenameExceptionFor(2).failure,
          MeetingRenameFailure.storageFailure);
      expect(meetingRenameExceptionFor(3).failure,
          MeetingRenameFailure.runtimeUnavailable);
      expect(meetingRenameExceptionFor(5).failure,
          MeetingRenameFailure.meetingNotFound);
      expect(
          meetingRenameExceptionFor(42).failure, MeetingRenameFailure.unknown);
    });

    test('keeps the raw status code for diagnostics', () {
      expect(meetingRenameExceptionFor(42).statusCode, 42);
      expect(meetingRenameExceptionFor(2).toString(), contains('2'));
    });
  });

  group('display formatting', () {
    test('formats durations as M:SS and H:MM:SS', () {
      expect(formatMeetingDuration(0), '0:00');
      expect(formatMeetingDuration(65999), '1:05');
      expect(formatMeetingDuration(3723000), '1:02:03');
    });

    test('formats creation times in UTC', () {
      expect(formatMeetingCreatedAt(DateTime.utc(2026, 1, 5, 4, 7)),
          '2026-01-05 04:07 UTC');
    });
  });
}
