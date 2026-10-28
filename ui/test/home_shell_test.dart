// Defines widget-first requirements for the responsive Material 3 navigation shell.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/home_shell.dart';

/// Pumps the shell at `size` (logical pixels) with placeholder destination screens.
Future<void> pumpHomeShell(WidgetTester tester, Size size) async {
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
        telemetryScreenBuilder: (BuildContext context) =>
            const Text('Telemetry screen'),
        devicesScreenBuilder: (BuildContext context) =>
            const Text('Devices screen'),
      ),
    ),
  );
}

/// Verifies phones use a bottom navigation bar and desktops a navigation rail.
void main() {
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

    await tester.tap(find.text('Telemetry'));
    await tester.pumpAndSettle();

    expect(find.text('Telemetry screen'), findsOneWidget);
  });
}
