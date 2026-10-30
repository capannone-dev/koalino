// Specifies the relay and static peer fields in Settings, and that the core's answer reaches them.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/network_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'fake_p2p_bridge.dart';

const String relayAddress = '/ip4/203.0.113.5/tcp/4001/p2p/12D3KooWRelay';

/// Pumps a section over a loaded controller.
Future<NetworkSettingsController> pumpSection(
    WidgetTester tester, FakeP2pBridge bridge) async {
  final NetworkSettingsController controller =
      NetworkSettingsController(p2pBridge: bridge);
  await controller.load();
  await tester.pumpWidget(MaterialApp(
    home: Scaffold(
      body: ListenableBuilder(
        listenable: controller,
        builder: (BuildContext context, Widget? child) => ListView(
          children: <Widget>[NetworkAddressesSection(controller: controller)],
        ),
      ),
    ),
  ));
  return controller;
}

void main() {
  testWidgets(
      'saves the fields, reports that it applies now, and shows the relay state',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge =
        FakeP2pBridge(status: const P2pStatus(enabled: true));
    await pumpSection(tester, bridge);

    await tester.enterText(find.byKey(relayAddressFieldKey), ' $relayAddress ');
    await tester.enterText(
        find.byKey(peerAddressesFieldKey), '/ip4/1.2.3.4/tcp/1/p2p/a\n\n');
    await tester.tap(find.text('Save addresses'));
    await tester.pumpAndSettle();

    expect(bridge.savedAddresses.single.relay, relayAddress);
    expect(bridge.savedAddresses.single.peers,
        <String>['/ip4/1.2.3.4/tcp/1/p2p/a']);
    expect(
        find.text('Saved. This applies now, with no restart.'), findsOneWidget);
    expect(find.textContaining('Not paired yet'), findsOneWidget);
  });

  testWidgets('a blank relay field removes the relay',
      (WidgetTester tester) async {
    final FakeP2pBridge bridge = FakeP2pBridge(
        status: const P2pStatus(
            enabled: true,
            network: NetworkRoutes(
                relayAddress: relayAddress, relayState: 'reserved')));
    await pumpSection(tester, bridge);
    expect(find.text(relayAddress), findsOneWidget);
    expect(find.textContaining('Your other devices can reach'), findsOneWidget);

    await tester.enterText(find.byKey(relayAddressFieldKey), '   ');
    await tester.tap(find.text('Save addresses'));
    await tester.pumpAndSettle();

    expect(bridge.savedAddresses.single.relay, isNull);
  });

  for (final (String code, String expected) in <(String, String)>[
    ('invalid_relay_address', 'That relay address is not valid'),
    ('invalid_peer_address', 'A device address is not valid'),
    ('unavailable', 'Device sync is not running.'),
    ('storage_failed', 'Unable to save the addresses.'),
  ]) {
    testWidgets('a refusal with $code is explained and saves nothing',
        (WidgetTester tester) async {
      final FakeP2pBridge bridge =
          FakeP2pBridge(status: const P2pStatus(enabled: true))
            ..nextCommandError = P2pException(code, 'rust text');
      await pumpSection(tester, bridge);

      await tester.enterText(find.byKey(relayAddressFieldKey), 'nonsense');
      await tester.tap(find.text('Save addresses'));
      await tester.pumpAndSettle();

      expect(find.textContaining(expected), findsOneWidget);
      expect(find.textContaining('rust text'), findsNothing);
      expect(bridge.savedAddresses, isEmpty);
    });
  }

  test('the fields are not offered when device sync is off', () async {
    final NetworkSettingsController controller = NetworkSettingsController(
        p2pBridge: FakeP2pBridge(status: const P2pStatus(enabled: false)));
    await controller.load();

    expect(controller.available, isFalse);
  });
}
