// Specifies how meetings' times are shown (#210): in the user's own time zone and words, relative
// to today, never as a UTC timestamp.

import 'package:flutter_test/flutter_test.dart';
import 'package:intl/intl.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_time.dart';

void main() {
  // Local wall-clock times, so the expectations hold in any time zone.
  final DateTime now = DateTime(2026, 11, 28, 18, 0);
  String time(DateTime local) => DateFormat.jm('en').format(local);

  test('today and yesterday are named', () {
    final DateTime morning = DateTime(2026, 11, 28, 9, 32);
    expect(formatMeetingWhen(morning, now: now), 'Today ${time(morning)}');
    final DateTime lastNight = DateTime(2026, 11, 27, 23, 50);
    expect(
        formatMeetingWhen(lastNight, now: now), 'Yesterday ${time(lastNight)}');
  });

  test(
      'this week shows the weekday, this year the day and month, older the year',
      () {
    final DateTime monday = DateTime(2026, 11, 23, 14, 0);
    expect(formatMeetingWhen(monday, now: now), 'Mon ${time(monday)}');
    final DateTime october = DateTime(2026, 10, 28, 8, 5);
    expect(formatMeetingWhen(october, now: now), 'Oct 28 ${time(october)}');
    final DateTime lastYear = DateTime(2025, 3, 2, 8, 5);
    expect(
        formatMeetingWhen(lastYear, now: now), 'Mar 2, 2025 ${time(lastYear)}');
  });

  test('a UTC instant is shown in local time, with no UTC label', () {
    final DateTime instant = DateTime.utc(2026, 11, 28, 8, 32);
    final String shown = formatMeetingWhen(instant, now: now);
    expect(shown, isNot(contains('UTC')));
    expect(shown, contains(time(instant.toLocal())));
  });

  test('calendar days are local and worded', () {
    expect(formatCalendarDay(DateTime(2026, 12, 19, 10)), 'Dec 19, 2026');
  });
}
