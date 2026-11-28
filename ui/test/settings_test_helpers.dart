// Shared steps for tests of the settings pages (#209).

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

/// Opens Settings' Advanced page, where transcription, storage, privacy and diagnostics live.
Future<void> openAdvancedSettings(WidgetTester tester) async {
  await tester.pumpAndSettle();
  await tester.tap(find.byKey(advancedSettingsKey));
  await tester.pumpAndSettle();
}
