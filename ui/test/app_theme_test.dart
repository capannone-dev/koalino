// Specifies the app's light and dark Material 3 themes and that the app follows the system setting.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/main.dart';
import 'package:note_taker_ui/presentation/app_theme.dart';

import 'recording_screen_test.dart' show FakeRecordingBridge;

void main() {
  test('both themes share the seed and differ only in brightness', () {
    final ThemeData light = noteTakerTheme(Brightness.light);
    final ThemeData dark = noteTakerTheme(Brightness.dark);

    expect(light.useMaterial3, isTrue);
    expect(light.colorScheme.brightness, Brightness.light);
    expect(dark.colorScheme.brightness, Brightness.dark);
    expect(dark.colorScheme.primary, isNot(light.colorScheme.primary),
        reason: 'the dark scheme uses the tones meant for dark surfaces');
  });

  for (final Brightness systemBrightness in Brightness.values) {
    testWidgets('the app follows a ${systemBrightness.name} system setting',
        (WidgetTester tester) async {
      tester.platformDispatcher.platformBrightnessTestValue = systemBrightness;
      addTearDown(tester.platformDispatcher.clearPlatformBrightnessTestValue);

      await tester.pumpWidget(NoteTakerApp(
          recordingController: RecordingController(
              recordingBridge: FakeRecordingBridge(startResult: true))));

      final BuildContext shellContext = tester.element(
          find.byType(NavigationBar).evaluate().isNotEmpty
              ? find.byType(NavigationBar)
              : find.byType(NavigationRail));
      expect(Theme.of(shellContext).brightness, systemBrightness);
    });
  }
}
