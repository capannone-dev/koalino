// Defines widget-first requirements for the responsive Material 3 navigation shell.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/home_shell.dart';

/// Pumps the shell at `size` (logical pixels) with placeholder destination screens, and a
/// Settings placeholder when `withSettings` is true.
Future<void> pumpHomeShell(WidgetTester tester, Size size,
    {bool withSettings = false}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      home: HomeShell(
        recordingScreenBuilder: (BuildContext context) =>
            const Text('Recording screen'),
        meetingsScreenBuilder: (BuildContext context) =>
            const Text('Meetings screen'),
        devicesScreenBuilder: (BuildContext context) =>
            const Text('Devices screen'),
        settingsScreenBuilder: withSettings
            ? (BuildContext context) => const Text('Settings screen')
            : null,
      ),
    ),
  );
}

/// Verifies phones use a bottom navigation bar and desktops a navigation rail.
void main() {
  testWidgets('Settings is a destination only when the app has settings',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(390, 844));
    expect(find.text('Settings'), findsNothing);

    await pumpHomeShell(tester, const Size(390, 844), withSettings: true);
    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();

    expect(find.text('Settings screen'), findsOneWidget);
  });

  testWidgets('a phone switches destinations with a bottom navigation bar',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(390, 844));

    expect(find.byType(NavigationBar), findsOneWidget);
    expect(find.byType(NavigationRail), findsNothing);
    expect(find.text('Recording screen'), findsOneWidget);

    await tester.tap(find.text('Meetings'));
    await tester.pumpAndSettle();

    expect(find.text('Meetings screen'), findsOneWidget);
    expect(find.text('Recording screen'), findsNothing);

    await tester.tap(find.text('Devices'));
    await tester.pumpAndSettle();

    expect(find.text('Devices screen'), findsOneWidget);
  });

  testWidgets('a desktop window switches destinations with a navigation rail',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(1280, 800));

    expect(find.byType(NavigationRail), findsOneWidget);
    expect(find.byType(NavigationBar), findsNothing);

    await tester.tap(find.text('Devices'));
    await tester.pumpAndSettle();

    expect(find.text('Devices screen'), findsOneWidget);
  });

  testWidgets('Telemetry is not a destination; it lives in Settings',
      (WidgetTester tester) async {
    for (final Size size in <Size>[
      const Size(390, 844),
      const Size(1280, 800),
    ]) {
      await pumpHomeShell(tester, size, withSettings: true);

      expect(find.text('Telemetry'), findsNothing);
      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      expect(find.text('Settings screen'), findsOneWidget);
    }
  });
}
