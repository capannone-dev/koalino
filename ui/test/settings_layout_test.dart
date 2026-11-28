// Specifies how Settings is laid out (#209): the settings most people change on the first page,
// and everything else behind Advanced, grouped under section headers.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'settings_test_helpers.dart';
import 'transcription_route_test.dart' show newLogSettingsController;

void main() {
  testWidgets('the first page keeps the basics and leads to Advanced',
      (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: SettingsScreen(
        logSettingsController: newLogSettingsController(),
        devicesScreenBuilder: (BuildContext context) =>
            const Text('Devices page'),
      ),
    ));
    await tester.pumpAndSettle();

    expect(find.text('Devices'), findsOneWidget);
    expect(find.text('Advanced'), findsOneWidget);
    expect(find.text('Log level'), findsNothing,
        reason: 'diagnostics are not a first-page setting');

    await openAdvancedSettings(tester);
    expect(find.widgetWithText(SettingsSectionHeader, 'Developer'),
        findsOneWidget);
    expect(find.text('Log level'), findsOneWidget);
    expect(find.widgetWithText(SettingsSectionHeader, 'Storage'), findsNothing,
        reason: 'a section with nothing in it is left out');
  });
}
