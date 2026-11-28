// Specifies the transcription choice in Settings: it appears only with device sync on, offers this
// device, the best device and paired devices, applies a choice, and keeps the old one if refused.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/controllers/transcription_route_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'fake_log_bridge.dart';
import 'fake_p2p_bridge.dart';
import 'settings_test_helpers.dart';

/// A status with one paired device.
P2pStatus _status({String route = 'local'}) => P2pStatus(
      enabled: true,
      transcriptionRoute: route,
      pairedDevices: <PairedDevice>[
        PairedDevice(
          peerId: '12D3KooWLaptop',
          deviceName: 'Laptop',
          pairedAt: DateTime.utc(2026),
        ),
      ],
    );

/// A log controller the Settings screen can render with.
LogSettingsController newLogSettingsController() => LogSettingsController(
      logBridge: FakeLogBridge(),
      exportSaver: FakeDesktopSaver(chosenPath: null),
      sharedDirectory: () async => Directory.systemTemp,
    );

/// A loaded controller over `bridge`.
Future<TranscriptionRouteController> _loaded(FakeP2pBridge bridge) async {
  final TranscriptionRouteController controller =
      TranscriptionRouteController(p2pBridge: bridge);
  await controller.load();
  return controller;
}

void main() {
  test('offers this device, the best device and each paired device', () async {
    final TranscriptionRouteController controller =
        await _loaded(FakeP2pBridge(status: _status()));

    expect(controller.available, isTrue);
    expect(controller.route, 'local');
    expect(
        controller.choices
            .map((TranscriptionRouteChoice choice) => choice.label),
        <String>['This device', 'Best available', 'Laptop']);
  });

  test('a stored device that was removed stays selectable', () async {
    final TranscriptionRouteController controller =
        await _loaded(FakeP2pBridge(status: _status(route: '12D3KooWGone')));

    expect(controller.route, '12D3KooWGone');
    expect(controller.choices.last.label, 'A removed device');
  });

  test('is unavailable with device sync off', () async {
    final TranscriptionRouteController controller =
        await _loaded(FakeP2pBridge(status: const P2pStatus(enabled: false)));

    expect(controller.available, isFalse);
  });

  test('applies a choice', () async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: _status());
    final TranscriptionRouteController controller = await _loaded(bridge);

    await controller.choose('12D3KooWLaptop');

    expect(bridge.routes, <String>['12D3KooWLaptop']);
    expect(controller.route, '12D3KooWLaptop');
    expect(controller.message, isNull);
  });

  test('keeps the old choice when the core refuses', () async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: _status());
    final TranscriptionRouteController controller = await _loaded(bridge);
    bridge.nextCommandError = const P2pException('refused', 'not paired');

    await controller.choose('12D3KooWLaptop');

    expect(controller.route, 'local');
    expect(controller.message, isNotNull);
    expect(controller.isSaving, isFalse);
  });

  testWidgets('Settings shows the choice only with device sync on',
      (WidgetTester tester) async {
    final LogSettingsController logs = newLogSettingsController();
    final TranscriptionRouteController on =
        await _loaded(FakeP2pBridge(status: _status()));
    await tester.pumpWidget(MaterialApp(
        home: SettingsScreen(
            logSettingsController: logs, transcriptionRouteController: on)));
    await openAdvancedSettings(tester);
    expect(find.text('Transcribe on'), findsOneWidget);
    expect(find.text('This device'), findsOneWidget);

    final TranscriptionRouteController off =
        await _loaded(FakeP2pBridge(status: const P2pStatus(enabled: false)));
    await tester.pumpWidget(MaterialApp(
        // A fresh app, not the Advanced page already open over the first one.
        key: UniqueKey(),
        home: SettingsScreen(
            logSettingsController: logs, transcriptionRouteController: off)));
    await openAdvancedSettings(tester);
    expect(find.text('Transcribe on'), findsNothing);
  });
}
