// Specifies pairing from an invite link a phone's camera opened (Issue #122): the owner is asked
// first, Pair finds the inviting device and starts pairing on the Devices screen, a refused
// invite says why, and Cancel does nothing.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/infrastructure/pairing_link_bridge.dart';
import 'package:note_taker_ui/presentation/pairing_link_listener.dart';

import 'fake_p2p_bridge.dart';

const String _link =
    'note-taker-invite:1;12D3KooWInviter;5000;/ip4/192.168.1.5/tcp/4001';

/// Links handed out in order.
final class _FakeLinkBridge implements PairingLinkBridge {
  final List<String> pending = <String>[];
  final StreamController<void> arrivalEvents =
      StreamController<void>.broadcast();

  @override
  Stream<void> get arrivals => arrivalEvents.stream;

  @override
  Future<String?> takePending() async =>
      pending.isEmpty ? null : pending.removeAt(0);
}

/// Shows the listener over a placeholder home.
Future<void> _pump(
    WidgetTester tester, _FakeLinkBridge links, FakeP2pBridge p2p) async {
  await tester.pumpWidget(MaterialApp(
    home: PairingLinkListener(
      linkBridge: links,
      devicesController: () => DevicesController(p2pBridge: p2p),
      child: const Scaffold(body: Text('Home')),
    ),
  ));
  await tester.pumpAndSettle();
}

/// Leaves the Devices screen, so its polling stops.
Future<void> _leave(WidgetTester tester) async {
  await tester.pageBack();
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('a link that opened the app asks, then starts pairing',
      (WidgetTester tester) async {
    final _FakeLinkBridge links = _FakeLinkBridge()..pending.add(_link);
    final FakeP2pBridge p2p = FakeP2pBridge(status: busyStatus());
    await _pump(tester, links, p2p);

    expect(find.text('Pair with another device?'), findsOneWidget);
    expect(p2p.joinedInvites, isEmpty, reason: 'nothing happens before Pair');
    await tester.tap(find.text('Pair'));
    await tester.pumpAndSettle();

    expect(p2p.joinedInvites, <String>[_link]);
    expect(p2p.commands,
        contains((command: 'start_pairing', peerId: '12D3KooWInviter')));
    expect(find.text('Devices'), findsOneWidget);
    expect(
        find.text(
            'Pairing started. Check that both devices show the same code.'),
        findsOneWidget);
    await _leave(tester);
  });

  testWidgets('a link arriving while the app runs is offered too',
      (WidgetTester tester) async {
    final _FakeLinkBridge links = _FakeLinkBridge();
    final FakeP2pBridge p2p = FakeP2pBridge(status: busyStatus());
    await _pump(tester, links, p2p);
    expect(find.text('Pair with another device?'), findsNothing);

    links.pending.add(_link);
    links.arrivalEvents.add(null);
    await tester.pumpAndSettle();

    expect(find.text('Pair with another device?'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();
    expect(p2p.joinedInvites, isEmpty);
    expect(find.text('Home'), findsOneWidget);
  });

  testWidgets('an expired invite says so on the Devices screen',
      (WidgetTester tester) async {
    final _FakeLinkBridge links = _FakeLinkBridge()..pending.add(_link);
    final FakeP2pBridge p2p = FakeP2pBridge(status: busyStatus())
      ..nextCommandError =
          const P2pException('invite_expired', 'The invite has expired.');
    await _pump(tester, links, p2p);

    await tester.tap(find.text('Pair'));
    await tester.pumpAndSettle();

    expect(
        find.text(
            'This invite has expired. Make a new one on the other device.'),
        findsOneWidget);
    expect(
        p2p.commands
            .where((P2pCommandCall call) => call.command == 'start_pairing'),
        isEmpty);
    await _leave(tester);
  });
}
