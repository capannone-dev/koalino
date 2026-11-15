// Gives every part of the app the text of the language in use (Issue #124), without a
// BuildContext. The root MaterialApp sets it from the resolved locale before its screens build;
// until then, and in tests that build widgets on their own, the text is English.

import 'dart:async';

import 'package:flutter/widgets.dart';
import 'package:intl/date_symbol_data_local.dart';
import 'package:intl/intl.dart';
import 'package:note_taker_ui/l10n/app_localizations.dart';

export 'package:note_taker_ui/l10n/app_localizations.dart';

AppLocalizations _current = lookupAppLocalizations(const Locale('en'));

/// The app's text in the language in use.
///
/// Inputs: None. Outputs: The translations last set by [useAppLocalizations], English before.
/// Side effects: None.
AppLocalizations get l10n => _current;

/// Makes `localizations` the text [l10n] returns.
///
/// Inputs: The translations of the locale the app resolved. Outputs: Whether the language changed.
/// Side effects: Replaces the process-wide translations; widgets built afterwards use them.
bool useAppLocalizations(AppLocalizations localizations) {
  if (_current.localeName == localizations.localeName) {
    return false;
  }
  _current = localizations;
  return true;
}

bool _dateFormattingReady = false;

/// Formats `time` with an `intl` date pattern in the app's language, as `d MMM y`.
///
/// Inputs: The time (already local) and the skeleton or pattern. Outputs: The formatted date.
/// Side effects: Loads `intl`'s bundled date symbols on first use (synchronously, though the API
/// returns a future).
String formatDatePattern(DateTime time, String pattern) {
  if (!_dateFormattingReady) {
    unawaited(initializeDateFormatting());
    _dateFormattingReady = true;
  }
  return DateFormat(pattern, _current.localeName).format(time);
}
