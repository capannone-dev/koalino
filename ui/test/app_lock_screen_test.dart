// Specifies the app-lock gate (it hides the app until unlocked, without discarding the app's
// state, such as an active recording) and the App lock switch in Settings.

import 'dart:async';
import 'dart:io';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/app_lock_controller.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/presentation/app_lock_gate.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'fake_app_lock_bridge.dart';
import 'fake_log_bridge.dart';

/// Counts taps, standing in for app state that must survive a lock.
final class _Counter extends StatefulWidget {
  const _Counter();

  @override
  State<_Counter> createState() => _CounterState();
}

final class _CounterState extends State<_Counter> {
  int _taps = 0;

  @override
  Widget build(BuildContext context) => TextButton(
        onPressed: () => setState(() => _taps += 1),
        child: Text('Taps: $_taps'),
      );
}

/// Pumps the gate around the counter.
Future<void> pumpGate(WidgetTester tester, AppLockController controller) async {
  await tester.pumpWidget(MaterialApp(
    home: AppLockGate(
      controller: controller,
      child: const Scaffold(body: Center(child: _Counter())),
    ),
  ));
}

void main() {
  group('AppLockGate', () {
    testWidgets('shows nothing of the app while the setting is read',
        (WidgetTester tester) async {
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true)
        ..pendingAuthentication = Completer<bool>();
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);

      await pumpGate(tester, controller);

      expect(find.text('Taps: 0'), findsNothing);
    });

    testWidgets('a locked app prompts at once and opens when unlocked',
        (WidgetTester tester) async {
      final AppLockController controller =
          AppLockController(appLockBridge: FakeAppLockBridge(enabled: true));
      await pumpGate(tester, controller);
      await controller.initialize();
      await tester.pump();

      await tester.pumpAndSettle();

      expect(find.text('Koalino is locked'), findsNothing);
      expect(find.text('Taps: 0'), findsOneWidget);
    });

    testWidgets('after a cancelled prompt, Unlock prompts again',
        (WidgetTester tester) async {
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true)
        ..authenticationResults.add(false);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await pumpGate(tester, controller);
      await controller.initialize();
      await tester.pumpAndSettle();

      expect(find.text('Koalino is locked'), findsOneWidget);
      expect(find.text('Taps: 0'), findsNothing);

      await tester.tap(find.text('Unlock'));
      await tester.pumpAndSettle();

      expect(bridge.authenticationCount, 2);
      expect(find.text('Taps: 0'), findsOneWidget);
    });

    testWidgets('locking keeps the app state, such as a recording',
        (WidgetTester tester) async {
      DateTime now = DateTime.utc(2026, 10, 28, 12);
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge, clock: () => now);
      await pumpGate(tester, controller);
      await controller.initialize();
      await tester.pumpAndSettle();
      await tester.tap(find.text('Taps: 0'));
      await tester.pump();

      bridge.authenticationResults.add(false);
      controller.appHidden();
      now = now.add(lockAfterBackground);
      controller.appShown();
      await tester.pumpAndSettle();
      expect(find.text('Koalino is locked'), findsOneWidget);

      await tester.tap(find.text('Unlock'));
      await tester.pumpAndSettle();

      expect(find.text('Taps: 1'), findsOneWidget);
    });

    testWidgets('pushed screens and dialogs are covered too',
        (WidgetTester tester) async {
      DateTime now = DateTime.utc(2026, 10, 28, 12);
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge, clock: () => now);
      final GlobalKey<NavigatorState> navigatorKey =
          GlobalKey<NavigatorState>();
      await tester.pumpWidget(MaterialApp(
        navigatorKey: navigatorKey,
        builder: (BuildContext context, Widget? navigator) =>
            AppLockGate(controller: controller, child: navigator!),
        home: const Scaffold(body: Text('Home')),
      ));
      await controller.initialize();
      await tester.pumpAndSettle();
      unawaited(navigatorKey.currentState!.push(MaterialPageRoute<void>(
          builder: (BuildContext context) =>
              const Scaffold(body: Text('Speakers')))));
      await tester.pumpAndSettle();

      bridge.authenticationResults.add(false);
      controller.appHidden();
      now = now.add(lockAfterBackground);
      controller.appShown();
      await tester.pumpAndSettle();

      expect(find.text('Koalino is locked'), findsOneWidget);
      expect(find.text('Speakers'), findsNothing);
    });

    testWidgets('leaving the app for longer than the grace period locks it',
        (WidgetTester tester) async {
      DateTime now = DateTime.utc(2026, 10, 28, 12);
      final FakeAppLockBridge bridge = FakeAppLockBridge(enabled: true);
      final AppLockController controller =
          AppLockController(appLockBridge: bridge, clock: () => now);
      await pumpGate(tester, controller);
      await controller.initialize();
      await tester.pumpAndSettle();
      bridge.authenticationResults.add(false);

      for (final AppLifecycleState state in <AppLifecycleState>[
        AppLifecycleState.inactive,
        AppLifecycleState.hidden,
        AppLifecycleState.paused,
      ]) {
        tester.binding.handleAppLifecycleStateChanged(state);
      }
      now = now.add(lockAfterBackground);
      for (final AppLifecycleState state in <AppLifecycleState>[
        AppLifecycleState.hidden,
        AppLifecycleState.inactive,
        AppLifecycleState.resumed,
      ]) {
        tester.binding.handleAppLifecycleStateChanged(state);
      }
      await tester.pumpAndSettle();

      expect(find.text('Koalino is locked'), findsOneWidget);
    });
  });

  group('SettingsScreen', () {
    /// Pumps Settings over an initialized controller.
    Future<AppLockController> pumpSettings(
        WidgetTester tester, FakeAppLockBridge bridge) async {
      final AppLockController controller =
          AppLockController(appLockBridge: bridge);
      await controller.initialize();
      final LogSettingsController logSettings = LogSettingsController(
        logBridge: FakeLogBridge(),
        exportSaver: FakeDesktopSaver(),
        sharedDirectory: () async => Directory.systemTemp,
      );
      await tester.pumpWidget(MaterialApp(
          home: SettingsScreen(
              appLockController: controller,
              logSettingsController: logSettings)));
      return controller;
    }

    testWidgets('App lock is turned on after authenticating',
        (WidgetTester tester) async {
      final FakeAppLockBridge bridge = FakeAppLockBridge();
      await pumpSettings(tester, bridge);
      expect(tester.widget<SwitchListTile>(find.byType(SwitchListTile)).value,
          isFalse);

      await tester.tap(find.text('App lock'));
      await tester.pumpAndSettle();

      expect(bridge.storedValues, <bool>[true]);
      expect(tester.widget<SwitchListTile>(find.byType(SwitchListTile)).value,
          isTrue);
    });

    testWidgets('without a screen lock the switch explains and is disabled',
        (WidgetTester tester) async {
      await pumpSettings(tester, FakeAppLockBridge(deviceSecure: false));

      expect(find.textContaining('Set a screen lock'), findsOneWidget);
      expect(
          tester.widget<SwitchListTile>(find.byType(SwitchListTile)).onChanged,
          isNull);
    });

    testWidgets('an Android version without the prompt disables the switch',
        (WidgetTester tester) async {
      await pumpSettings(tester, FakeAppLockBridge(supported: false));

      expect(find.textContaining('Android 10'), findsOneWidget);
      expect(
          tester.widget<SwitchListTile>(find.byType(SwitchListTile)).onChanged,
          isNull);
    });
  });
}
