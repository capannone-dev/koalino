// Specifies the Devices screen: pairing codes in large type, the pairing actions, and removal.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/presentation/devices_screen.dart';

import 'fake_p2p_bridge.dart';

/// Pumps the screen at phone size over a controller that has loaded `bridge`'s status.
Future<DevicesController> pumpDevices(
    WidgetTester tester, FakeP2pBridge bridge) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(390, 844);
  addTearDown(tester.view.reset);
  final DevicesController controller = DevicesController(p2pBridge: bridge);
  await controller.refresh();
  await tester.pumpWidget(
      MaterialApp(home: Scaffold(body: DevicesScreen(controller: controller))));
  await tester.pumpAndSettle();
  return controller;
}

void main() {
  testWidgets('shows this device and every section of a busy status',
      (WidgetTester tester) async {
    await pumpDevices(tester, FakeP2pBridge(status: busyStatus()));

    expect(find.text('Studio laptop'), findsOneWidget);
    expect(find.text('Pairing requests'), findsOneWidget);
    expect(find.text('Tablet'), findsOneWidget);

    // Two pending pairings fill a phone screen; the lists follow below them.
    await tester.scrollUntilVisible(find.text('Galaxy'), 200);
    expect(find.text('Paired devices'), findsOneWidget);
    expect(find.text('Galaxy'), findsOneWidget);
    await tester.scrollUntilVisible(find.text('Kitchen tablet'), 200);
    expect(find.text('Nearby devices'), findsOneWidget);
    expect(find.text('Pair'), findsNWidgets(2));
  });

  testWidgets(
      'a nearby device shows its name, or its peer ID until it tells one (#121)',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus());
    await pumpDevices(tester, bridge);
    await tester.scrollUntilVisible(find.text('Kitchen tablet'), 200);

    expect(find.text('Kitchen tablet'), findsOneWidget);
    expect(find.text(shortPeerId('12D3KooWNearbyDevice1234')), findsOneWidget,
        reason:
            'the peer ID stays as a subtitle, to tell same-named devices apart');
    expect(find.text(shortPeerId('12D3KooWQuietDevice5678')), findsOneWidget);

    await tester.tap(find.text('Pair').first);
    await tester.pumpAndSettle();
    expect(bridge.commands.last,
        (command: 'start_pairing', peerId: '12D3KooWNearbyDevice1234'));
  });

  testWidgets('renaming this device saves the typed name (#121)',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus());
    await pumpDevices(tester, bridge);

    await tester.tap(find.byTooltip('Rename this device'));
    await tester.pumpAndSettle();
    expect(find.text('Rename this device'), findsOneWidget);
    expect(find.widgetWithText(TextField, 'Studio laptop'), findsOneWidget,
        reason: 'the field starts from the current name');
    await tester.enterText(find.byType(TextField), '  Ettore laptop ');
    await tester.tap(find.widgetWithText(FilledButton, 'Save'));
    await tester.pumpAndSettle();

    expect(bridge.deviceNames, <String>['  Ettore laptop ']);
    expect(find.text('Ettore laptop'), findsOneWidget);
    expect(find.text('Studio laptop'), findsNothing);
  });

  testWidgets('a refused name keeps the dialog open and says why (#121)',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus())
      ..nextCommandError = const P2pException('invalid_device_name', 'blank');
    await pumpDevices(tester, bridge);

    await tester.tap(find.byTooltip('Rename this device'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), ' ');
    await tester.tap(find.widgetWithText(FilledButton, 'Save'));
    await tester.pumpAndSettle();

    expect(find.text('Rename this device'), findsOneWidget);
    expect(find.text('Use 1 to 64 characters, without line breaks.'),
        findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(find.text('Studio laptop'), findsOneWidget);
  });

  testWidgets('shows what each device can run, or that it is not connected',
      (WidgetTester tester) async {
    await pumpDevices(tester, FakeP2pBridge(status: busyStatus()));

    expect(find.textContaining('8 CPU cores · Whisper (on device)'),
        findsOneWidget);
    await tester.scrollUntilVisible(find.text('Old tablet'), 200);
    expect(
        find.textContaining(
            '16 CPU cores · 8 GB GPU · Whisper (on device), OpenAI (cloud)'),
        findsOneWidget);
    expect(find.textContaining('Not connected'), findsOneWidget);
  });

  test('capabilities are summarized in one line', () {
    expect(
      describeCapabilities(const DeviceCapabilities(
          cpuCores: 1,
          gpuVramMegabytes: 512,
          transcriptionModels: <String>['acme'])),
      '1 CPU core · 512 MB GPU · acme',
    );
    expect(
      describeCapabilities(const DeviceCapabilities(
          cpuCores: 4, gpuVramMegabytes: 0, transcriptionModels: <String>[])),
      '4 CPU cores · No transcription models',
    );
  });

  testWidgets('pairing codes are shown in large type',
      (WidgetTester tester) async {
    await pumpDevices(tester, FakeP2pBridge(status: busyStatus()));

    final Text incomingCode = tester.widget<Text>(find.text('639 210'));
    final Text outgoingCode = tester.widget<Text>(find.text('888 224'));
    final double bodySize = Theme.of(tester.element(find.text('Tablet')))
            .textTheme
            .bodyLarge
            ?.fontSize ??
        16;

    expect(incomingCode.style?.fontSize, greaterThan(bodySize * 1.5));
    expect(outgoingCode.style?.fontSize, greaterThan(bodySize * 1.5));
    expect(find.bySemanticsLabel(RegExp('Pairing code 6 3 9 2 1 0')),
        findsOneWidget);
  });

  testWidgets(
      'accepting a request and confirming a pairing send their commands',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus());
    await pumpDevices(tester, bridge);

    await tester.tap(find.widgetWithText(FilledButton, 'Accept'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Codes match'));
    await tester.pumpAndSettle();

    expect(bridge.commands, <P2pCommandCall>[
      (command: 'accept_request', peerId: '12D3KooWTablet'),
      (command: 'confirm_code', peerId: '12D3KooWDesk'),
    ]);
  });

  testWidgets('a confirmed pairing waits for the other device',
      (WidgetTester tester) async {
    final P2pStatus waiting = P2pStatus(
      enabled: true,
      localPeerId: '12D3KooWLocal',
      deviceName: 'Studio laptop',
      outgoingPairings: const <OutgoingPairing>[
        OutgoingPairing(
            peerId: '12D3KooWDesk', code: '888224', locallyConfirmed: true),
      ],
    );
    await pumpDevices(tester, FakeP2pBridge(status: waiting));

    expect(find.text('Waiting for the other device…'), findsOneWidget);
    expect(find.widgetWithText(FilledButton, 'Codes match'), findsNothing);
  });

  testWidgets('removing a paired device asks first',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus());
    await pumpDevices(tester, bridge);
    await tester.scrollUntilVisible(find.byTooltip('Remove Galaxy'), 200);
    // Paired tiles are two lines tall; bring the whole button on screen before tapping it.
    await tester.ensureVisible(find.byTooltip('Remove Galaxy'));
    await tester.pumpAndSettle();

    await tester.tap(find.byTooltip('Remove Galaxy'));
    await tester.pumpAndSettle();
    expect(find.text('Remove Galaxy?'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(bridge.commands, isEmpty);

    await tester.tap(find.byTooltip('Remove Galaxy'));
    await tester.pumpAndSettle();
    await tester.tap(find.widgetWithText(FilledButton, 'Remove'));
    await tester.pumpAndSettle();
    expect(bridge.commands,
        <P2pCommandCall>[(command: 'revoke', peerId: '12D3KooWPhone')]);
  });

  testWidgets('make discoverable opens pairing and says so',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: emptyStatus());
    await pumpDevices(tester, bridge);
    expect(find.textContaining('No other devices found'), findsOneWidget);

    await tester.tap(find.text('Make discoverable'));
    await tester.pumpAndSettle();

    expect(bridge.commands.single.command, 'make_pairable');
    expect(find.textContaining('Discoverable'), findsOneWidget);
  });

  testWidgets('a disabled task explains that sync is off',
      (WidgetTester tester) async {
    await pumpDevices(
        tester, FakeP2pBridge(status: const P2pStatus(enabled: false)));

    expect(find.text('Device sync is off'), findsOneWidget);
    expect(find.text('Make discoverable'), findsNothing);
  });
}
