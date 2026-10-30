// Specifies what the Devices screen says about devices on other networks (Issue #66) and its
// pairing invites (Issue #65).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/presentation/devices_screen.dart';

import 'fake_p2p_bridge.dart';

const NetworkRoutes withRelay = NetworkRoutes(
    relayAddress: '/ip4/203.0.113.5/tcp/4001/p2p/12D3KooWRelay',
    relayState: 'reserved');

/// Pumps the Devices screen at desktop size over `bridge`'s status.
Future<void> pumpDevices(WidgetTester tester, FakeP2pBridge bridge) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(1000, 1400);
  addTearDown(tester.view.reset);
  final DevicesController controller = DevicesController(p2pBridge: bridge);
  await controller.refresh();
  await tester
      .pumpWidget(MaterialApp(home: DevicesScreen(controller: controller)));
  await tester.pumpAndSettle();
}

/// A status with one paired device that is not connected, and nothing nearby.
P2pStatus disconnectedStatus(NetworkRoutes network) => P2pStatus(
      enabled: true,
      deviceName: 'Phone',
      network: network,
      pairedDevices: <PairedDevice>[
        PairedDevice(
          peerId: '12D3KooWLaptop',
          deviceName: 'Laptop',
          pairedAt: DateTime.utc(2026, 10, 28),
        ),
      ],
    );

void main() {
  group('without a relay', () {
    testWidgets('says what to do next', (WidgetTester tester) async {
      await pumpDevices(tester,
          FakeP2pBridge(status: disconnectedStatus(const NetworkRoutes())));

      expect(find.textContaining('Nearby only finds devices on the same Wi-Fi'),
          findsOneWidget);
      expect(find.textContaining('use an invite or set a relay in Settings'),
          findsOneWidget);
      expect(
          find.textContaining(
              'Put both devices on the same Wi-Fi, or set a relay in Settings'),
          findsOneWidget);
    });
  });

  group('with a relay', () {
    testWidgets('does not ask for one again', (WidgetTester tester) async {
      await pumpDevices(
          tester, FakeP2pBridge(status: disconnectedStatus(withRelay)));

      expect(find.textContaining('Nearby only finds devices on the same Wi-Fi'),
          findsOneWidget);
      expect(find.textContaining('set a relay in Settings'), findsNothing);
      expect(find.textContaining('can reach your relay'), findsOneWidget);
    });

    testWidgets('a device that is itself the relay counts',
        (WidgetTester tester) async {
      await pumpDevices(
          tester,
          FakeP2pBridge(
              status:
                  disconnectedStatus(const NetworkRoutes(offersRelay: true))));

      expect(find.textContaining('set a relay in Settings'), findsNothing);
    });
  });

  group('invites', () {
    testWidgets('Show invite offers the link and keeps the device discoverable',
        (WidgetTester tester) async {
      final FakeP2pBridge bridge =
          FakeP2pBridge(status: disconnectedStatus(const NetworkRoutes()));
      await pumpDevices(tester, bridge);

      await tester.tap(find.text('Show invite'));
      await tester.pumpAndSettle();

      expect(find.text(bridge.invite.text), findsOneWidget);
      expect(find.text('Copy link'), findsOneWidget);
      expect(find.bySemanticsLabel('Pairing invite QR code'), findsOneWidget);
      expect(find.textContaining('15:07'), findsOneWidget);
    });

    testWidgets('a device with no address reports why there is no invite',
        (WidgetTester tester) async {
      final FakeP2pBridge bridge =
          FakeP2pBridge(status: disconnectedStatus(const NetworkRoutes()))
            ..nextCommandError = const P2pException('no_addresses', 'x');
      await pumpDevices(tester, bridge);

      await tester.tap(find.text('Show invite'));
      await tester.pumpAndSettle();

      expect(find.byType(AlertDialog), findsNothing);
      expect(find.textContaining('not on a network yet'), findsOneWidget);
    });

    testWidgets('Use invite sends the text and says what comes next',
        (WidgetTester tester) async {
      final FakeP2pBridge bridge =
          FakeP2pBridge(status: disconnectedStatus(const NetworkRoutes()));
      await pumpDevices(tester, bridge);

      await tester.tap(find.text('Use invite'));
      await tester.pumpAndSettle();
      await tester.enterText(find.byType(TextField), 'invite text');
      await tester.tap(find.text('Find device'));
      await tester.pumpAndSettle();

      expect(bridge.joinedInvites, <String>['invite text']);
      expect(find.byType(AlertDialog), findsNothing);
      expect(find.textContaining('Choose Pair under Nearby devices'),
          findsOneWidget);
    });

    for (final (String code, String expected) in <(String, String)>[
      ('invite_malformed', 'not a valid invite'),
      ('invite_expired', 'has expired'),
      ('invite_from_this_device', 'made on this device'),
      ('already_paired', 'already paired'),
    ]) {
      testWidgets('a refused invite ($code) stays open with the reason',
          (WidgetTester tester) async {
        final FakeP2pBridge bridge =
            FakeP2pBridge(status: disconnectedStatus(const NetworkRoutes()))
              ..nextCommandError = P2pException(code, 'rust text');
        await pumpDevices(tester, bridge);

        await tester.tap(find.text('Use invite'));
        await tester.pumpAndSettle();
        await tester.enterText(find.byType(TextField), 'bad');
        await tester.tap(find.text('Find device'));
        await tester.pumpAndSettle();

        expect(find.byType(AlertDialog), findsOneWidget);
        expect(find.textContaining(expected), findsOneWidget);
        expect(bridge.joinedInvites, isEmpty);
      });
    }
  });
}
