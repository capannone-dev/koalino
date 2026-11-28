// Words a meeting's time for people (#210): in the device's time zone and language, relative to
// today ("Today 9:32 AM", "Yesterday …", "Mon …", "Oct 28 …", "Mar 2, 2025 …"). Untitled meetings'
// stored titles keep their UTC form (`formatMeetingCreatedAt`), since every device must agree on them.

import 'package:intl/date_symbol_data_local.dart';
import 'package:intl/intl.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Loads every language's date words, bundled with `intl`, the first time a date is worded. The
/// data is set synchronously; the returned future only reports completion.
final bool _loadDateWords = () {
  initializeDateFormatting();
  return true;
}();

/// Returns the app language's locale for `intl`, or English when its date words are not loaded.
///
/// Inputs: none (reads the current app language). Outputs: a locale name `DateFormat` accepts.
/// Side effects: none.
String dateLocale() {
  if (!_loadDateWords) return 'en';
  final String locale = l10n.localeName;
  try {
    DateFormat.jm(locale);
    return locale;
  } on Object {
    return 'en';
  }
}

/// Words `instant` relative to `now` (default: the current time), in local time.
///
/// Inputs: any instant (UTC or local); `now` only for tests.
/// Outputs: "Today <time>", "Yesterday <time>", the weekday within the last week, the day and
/// month within this year, else the full date; each followed by the time of day.
/// Side effects: none.
String formatMeetingWhen(DateTime instant, {DateTime? now}) {
  final String locale = dateLocale();
  final DateTime local = instant.toLocal();
  final DateTime current = (now ?? DateTime.now()).toLocal();
  final String time = DateFormat.jm(locale).format(local);
  // Whole calendar days between the two dates, immune to daylight-saving shifts.
  final int daysAgo = DateTime.utc(current.year, current.month, current.day)
      .difference(DateTime.utc(local.year, local.month, local.day))
      .inDays;
  if (daysAgo == 0) return l10n.todayAt(time);
  if (daysAgo == 1) return l10n.yesterdayAt(time);
  if (daysAgo > 1 && daysAgo < 7) {
    return '${DateFormat.E(locale).format(local)} $time';
  }
  if (local.year == current.year) {
    return '${DateFormat.MMMd(locale).format(local)} $time';
  }
  return '${DateFormat.yMMMd(locale).format(local)} $time';
}
