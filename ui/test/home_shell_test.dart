// Defines widget-first requirements for the responsive Material 3 navigation shell.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/home_shell.dart';

/// Pumps the shell at `size` (logical pixels) with placeholder destination screens, and a
/// Settings placeholder when `withSettings` is true and a Projects one when `withProjects` is.
Future<void> pumpHomeShell(WidgetTester tester, Size size,
    {bool withSettings = false, bool withProjects = false}) async {
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
        voicesScreenBuilder: (BuildContext context) =>
            const Text('Voices screen'),
        projectsScreenBuilder: withProjects
            ? (BuildContext context) => const Text('Projects screen')
            : null,
        settingsScreenBuilder: withSettings
            ? (BuildContext context) => const Text('Settings screen')
            : null,
      ),
    ),
  );
}

/// Verifies phones use a bottom navigation bar and desktops a navigation rail.
void main() {
  testWidgets('Projects sits between Meetings and Voices and opens its screen',
      (WidgetTester tester) async {
    for (final Size size in <Size>[
      const Size(390, 844),
      const Size(1280, 800),
    ]) {
      await pumpHomeShell(tester, size, withSettings: true, withProjects: true);

      final List<double> positions = <String>[
        'Record',
        'Meetings',
        'Projects',
        'Voices',
        'Settings',
      ]
          .map((String label) => size.width < 600
              ? tester.getCenter(find.text(label)).dx
              : tester.getCenter(find.text(label)).dy)
          .toList();
      expect(positions, List<double>.of(positions)..sort());

      await tester.tap(find.text('Projects'));
      await tester.pumpAndSettle();
      expect(find.text('Projects screen'), findsOneWidget);
      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      expect(find.text('Settings screen'), findsOneWidget);
    }
  });

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

    await tester.tap(find.text('Voices'));
    await tester.pumpAndSettle();

    expect(find.text('Voices screen'), findsOneWidget);
  });

  testWidgets('a desktop window switches destinations with a navigation rail',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(1280, 800));

    expect(find.byType(NavigationRail), findsOneWidget);
    expect(find.byType(NavigationBar), findsNothing);

    await tester.tap(find.text('Meetings'));
    await tester.pumpAndSettle();

    expect(find.text('Meetings screen'), findsOneWidget);
  });

  testWidgets(
      'a desktop window keeps Settings at the bottom of the rail (#142)',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(1280, 800),
        withSettings: true, withProjects: true);

    final double settingsTop = tester.getTopLeft(find.text('Settings')).dy;
    expect(settingsTop, greaterThan(700),
        reason: 'at the bottom of an 800 px window');
    expect(settingsTop - tester.getTopLeft(find.text('Voices')).dy,
        greaterThan(300),
        reason: 'apart from the other destinations');

    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();
    expect(find.text('Settings screen'), findsOneWidget);
    expect(
        tester
            .widget<NavigationRail>(find.byType(NavigationRail))
            .selectedIndex,
        isNull,
        reason: 'no other destination looks selected');

    await tester.tap(find.text('Voices'));
    await tester.pumpAndSettle();
    expect(find.text('Voices screen'), findsOneWidget);
  });

  testWidgets('Devices is not a destination; it lives in Settings (#84)',
      (WidgetTester tester) async {
    for (final Size size in <Size>[
      const Size(390, 844),
      const Size(1280, 800),
    ]) {
      await pumpHomeShell(tester, size, withSettings: true);

      expect(find.text('Devices'), findsNothing);
      expect(
          find.text('Record').evaluate().length +
              find.text('Meetings').evaluate().length +
              find.text('Voices').evaluate().length +
              find.text('Settings').evaluate().length,
          4);
    }
  });

  testWidgets(
      'the voice gallery opens from the top level on phones and desktops',
      (WidgetTester tester) async {
    for (final Size size in <Size>[
      const Size(390, 844),
      const Size(1280, 800),
    ]) {
      await pumpHomeShell(tester, size, withSettings: true);

      await tester.tap(find.text('Voices'));
      await tester.pumpAndSettle();
      expect(find.text('Voices screen'), findsOneWidget);

      await tester.tap(find.text('Settings'));
      await tester.pumpAndSettle();
      expect(find.text('Settings screen'), findsOneWidget);
    }
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
