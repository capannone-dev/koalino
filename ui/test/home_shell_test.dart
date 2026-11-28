// Defines widget-first requirements for the responsive Material 3 navigation shell: Meetings is
// home, Record is a button on every main screen rather than a destination, and a recording in
// progress stays one tap away from anywhere (#208).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/recording_notifier.dart';
import 'package:note_taker_ui/presentation/koalino_logo.dart';
import 'package:note_taker_ui/presentation/home_shell.dart';

import 'recording_screen_test.dart' show FakeRecordingBridge;

/// Pumps the shell at `size` (logical pixels) with placeholder destination screens, and a
/// Settings placeholder when `withSettings` is true and a Projects one when `withProjects` is.
/// The Meetings placeholder has a button that opens the recorder through the shell.
Future<void> pumpHomeShell(WidgetTester tester, Size size,
    {bool withSettings = false,
    bool withProjects = false,
    RecordingController? recordingController}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      // A fresh shell per size, so a destination chosen at one size does not carry over.
      key: ValueKey<Size>(size),
      home: HomeShell(
        recordingScreenBuilder: (BuildContext context) =>
            Scaffold(appBar: AppBar(), body: const Text('Recording screen')),
        meetingsScreenBuilder: (BuildContext context) => Column(
          children: <Widget>[
            const Text('Meetings screen'),
            TextButton(
              onPressed: () => HomeShell.openRecorder(context),
              child: const Text('Empty state: record'),
            ),
          ],
        ),
        voicesScreenBuilder: (BuildContext context) =>
            const Text('People screen'),
        projectsScreenBuilder: withProjects
            ? (BuildContext context) => const Text('Projects screen')
            : null,
        settingsScreenBuilder: withSettings
            ? (BuildContext context) => const Text('Settings screen')
            : null,
        recordingController: recordingController,
      ),
    ),
  );
}

/// A controller whose recording has started.
Future<RecordingController> recordingInProgress() async {
  final RecordingController controller = RecordingController(
    recordingBridge: FakeRecordingBridge(startResult: true),
    microphonePermission: const GrantedMicrophonePermission(),
    recordingNotifier: const NoRecordingNotifier(),
  );
  await controller.startRecording(title: 'Weekly sync');
  return controller;
}

/// Verifies phones use a bottom navigation bar and desktops a navigation rail.
void main() {
  testWidgets('the app opens on Meetings; Record is not a destination (#208)',
      (WidgetTester tester) async {
    for (final Size size in <Size>[
      const Size(390, 844),
      const Size(1280, 800),
    ]) {
      await pumpHomeShell(tester, size, withSettings: true, withProjects: true);
      expect(find.text('Meetings screen'), findsOneWidget);

      final List<double> positions = <String>[
        'Meetings',
        'Projects',
        'People',
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

  testWidgets('a phone has a Record button that opens the recorder (#208)',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(390, 844), withSettings: true);
    expect(find.byType(NavigationBar), findsOneWidget);
    expect(find.byType(NavigationRail), findsNothing);

    await tester.tap(find.widgetWithText(FloatingActionButton, 'Record'));
    await tester.pumpAndSettle();
    expect(find.text('Recording screen'), findsOneWidget);

    await tester.pageBack();
    await tester.pumpAndSettle();
    expect(find.text('Meetings screen'), findsOneWidget);

    await tester.tap(find.text('People'));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(FloatingActionButton, 'Record'), findsOneWidget,
        reason: 'Record is one tap away on every main screen');
    await tester.tap(find.text('Settings'));
    await tester.pumpAndSettle();
    expect(find.widgetWithText(FloatingActionButton, 'Record'), findsNothing,
        reason: 'not over the settings');
  });

  testWidgets('a desktop window has Record at the top of the rail (#208)',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(1280, 800));
    expect(find.byType(NavigationRail), findsOneWidget);
    expect(find.byType(NavigationBar), findsNothing);
    expect(find.byKey(koalinoLogoKey), findsOneWidget); // the logo (#172)

    await tester.tap(find.byKey(const ValueKey<String>('rail-record')));
    await tester.pumpAndSettle();
    expect(find.text('Recording screen'), findsOneWidget);
  });

  testWidgets('screens inside the shell can open the recorder (#204, #208)',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(390, 844));
    await tester.tap(find.text('Empty state: record'));
    await tester.pumpAndSettle();
    expect(find.text('Recording screen'), findsOneWidget);
  });

  testWidgets(
      'while recording, a bar names the recording, stops it, and reopens the recorder (#208)',
      (WidgetTester tester) async {
    for (final Size size in <Size>[
      const Size(390, 844),
      const Size(1280, 800),
    ]) {
      final RecordingController controller = await recordingInProgress();
      await pumpHomeShell(tester, size, recordingController: controller);

      expect(
          find.byKey(const ValueKey<String>('recording-bar')), findsOneWidget);
      expect(find.text('Weekly sync'), findsOneWidget);
      await tester.tap(find.text('Weekly sync'));
      await tester.pumpAndSettle();
      expect(find.text('Recording screen'), findsOneWidget);
      await tester.pageBack();
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('Pause'));
      await tester.pump();
      expect(controller.state, RecordingUiState.paused);
      await tester.tap(find.byTooltip('Stop'));
      await tester.pumpAndSettle();
      expect(find.byKey(const ValueKey<String>('recording-bar')), findsNothing);
    }
  });

  testWidgets(
      'a desktop window keeps Settings at the bottom of the rail (#142)',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, const Size(1280, 800),
        withSettings: true, withProjects: true);

    final double settingsTop = tester.getTopLeft(find.text('Settings')).dy;
    expect(settingsTop, greaterThan(700),
        reason: 'at the bottom of an 800 px window');
    expect(settingsTop - tester.getTopLeft(find.text('People')).dy,
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

    await tester.tap(find.text('People'));
    await tester.pumpAndSettle();
    expect(find.text('People screen'), findsOneWidget);
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

  testWidgets(
      'Devices and Telemetry are not destinations; they live in Settings',
      (WidgetTester tester) async {
    for (final Size size in <Size>[
      const Size(390, 844),
      const Size(1280, 800),
    ]) {
      await pumpHomeShell(tester, size, withSettings: true);
      expect(find.text('Devices'), findsNothing);
      expect(find.text('Telemetry'), findsNothing);
    }
  });
}
