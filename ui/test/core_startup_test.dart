// Specifies start-up of the Rust core: the app waits for it, and a keychain or storage failure
// shows an explanation with Retry and Quit instead of the app (no key-file fallback).

import 'dart:async';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/core_startup_controller.dart';
import 'package:note_taker_ui/presentation/core_startup_gate.dart';

/// Answers each start with the next scripted status and counts the calls.
final class ScriptedStart {
  ScriptedStart(this.statuses);

  final List<int> statuses;
  int calls = 0;

  Future<int> call() async => statuses[calls++];
}

Future<void> pumpGate(WidgetTester tester, CoreStartupController controller,
    {VoidCallback? onQuit}) async {
  await tester.pumpWidget(MaterialApp(
    home: CoreStartupGate(
      controller: controller,
      onQuit: onQuit ?? () {},
      child: const Text('The app'),
    ),
  ));
}

void main() {
  test('maps the core statuses to states', () async {
    final ScriptedStart start = ScriptedStart(<int>[6, 2, 0]);
    final CoreStartupController controller =
        CoreStartupController(startCore: start.call);
    expect(controller.state, CoreStartupState.starting);

    await controller.start();
    expect(controller.state, CoreStartupState.keychainUnavailable);
    await controller.start();
    expect(controller.state, CoreStartupState.failed);
    await controller.start();
    expect(controller.state, CoreStartupState.ready);
  });

  test('a start that throws, such as a missing library, counts as failed',
      () async {
    final CoreStartupController controller = CoreStartupController(
        startCore: () async => throw ArgumentError('no library'));

    await controller.start();

    expect(controller.state, CoreStartupState.failed);
  });

  testWidgets('waits while the core starts, then shows the app',
      (WidgetTester tester) async {
    final Completer<int> pendingStart = Completer<int>();
    final CoreStartupController controller =
        CoreStartupController(startCore: () => pendingStart.future);
    await pumpGate(tester, controller);
    unawaited(controller.start());
    await tester.pump();

    expect(find.textContaining('Unlocking'), findsOneWidget);
    expect(find.text('The app'), findsNothing);

    pendingStart.complete(0);
    await tester.pumpAndSettle();
    expect(find.text('The app'), findsOneWidget);
  });

  testWidgets('a keychain failure explains itself and Retry starts again',
      (WidgetTester tester) async {
    final ScriptedStart start = ScriptedStart(<int>[6, 0]);
    final CoreStartupController controller =
        CoreStartupController(startCore: start.call);
    await pumpGate(tester, controller);
    await controller.start();
    await tester.pumpAndSettle();

    expect(find.text('Unable to open the keychain'), findsOneWidget);
    expect(find.textContaining('keychain'), findsWidgets);
    expect(find.text('The app'), findsNothing);

    await tester.tap(find.widgetWithText(FilledButton, 'Retry'));
    await tester.pumpAndSettle();

    expect(start.calls, 2);
    expect(find.text('The app'), findsOneWidget);
  });

  testWidgets('Quit calls the quit action', (WidgetTester tester) async {
    bool quitRequested = false;
    final CoreStartupController controller =
        CoreStartupController(startCore: ScriptedStart(<int>[2]).call);
    await pumpGate(tester, controller, onQuit: () => quitRequested = true);
    await controller.start();
    await tester.pumpAndSettle();

    expect(find.text('Unable to open your notes'), findsOneWidget);
    await tester.tap(find.text('Quit'));

    expect(quitRequested, isTrue);
  });
}
