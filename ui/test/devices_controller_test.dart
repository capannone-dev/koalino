// Specifies the Devices controller: it loads the P2P status, runs pairing commands, and refreshes.

import 'package:fake_async/fake_async.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';

import 'fake_p2p_bridge.dart';

void main() {
  test('refresh loads the status and reports a failure without details',
      () async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus());
    final DevicesController controller = DevicesController(p2pBridge: bridge);

    await controller.refresh();
    expect(controller.status?.deviceName, 'Studio laptop');
    expect(controller.errorMessage, isNull);

    bridge.statusError = const P2pException('runtime_unavailable', 'at /path');
    await controller.refresh();
    expect(controller.errorMessage, 'Unable to load devices');
    expect(controller.status?.deviceName, 'Studio laptop',
        reason: 'the last good status stays visible');
  });

  test('each action sends its command and peer, then refreshes', () async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus());
    final DevicesController controller = DevicesController(p2pBridge: bridge);

    await controller.makeDiscoverable();
    await controller.startPairing('12D3KooWNearby');
    await controller.confirmCode('12D3KooWDesk');
    await controller.cancelPairing('12D3KooWDesk');
    await controller.acceptRequest('12D3KooWTablet');
    await controller.rejectRequest('12D3KooWTablet');
    await controller.removeDevice('12D3KooWPhone');

    expect(bridge.commands, <P2pCommandCall>[
      (command: 'make_pairable', peerId: null),
      (command: 'start_pairing', peerId: '12D3KooWNearby'),
      (command: 'confirm_code', peerId: '12D3KooWDesk'),
      (command: 'cancel_pairing', peerId: '12D3KooWDesk'),
      (command: 'accept_request', peerId: '12D3KooWTablet'),
      (command: 'reject_request', peerId: '12D3KooWTablet'),
      (command: 'revoke', peerId: '12D3KooWPhone'),
    ]);
    expect(bridge.statusRequestCount, 7);
    expect(controller.discoverable, isTrue);
  });

  test('a refused command shows a readable message and keeps the status',
      () async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus())
      ..nextCommandError =
          const P2pException('refused', 'no pairing with that peer');
    final DevicesController controller = DevicesController(p2pBridge: bridge);
    await controller.refresh();

    await controller.confirmCode('12D3KooWDesk');

    expect(controller.errorMessage,
        'The other device is no longer pairing. Try again.');
    expect(controller.status?.outgoingPairings, isNotEmpty);
  });

  test('a disabled or stopped P2P task reports that sync is off', () async {
    final FakeP2pBridge bridge = FakeP2pBridge(status: busyStatus())
      ..nextCommandError = const P2pException('unavailable', 'stopped');
    final DevicesController controller = DevicesController(p2pBridge: bridge);

    await controller.makeDiscoverable();

    expect(controller.errorMessage, 'Device sync is not running.');
    expect(controller.discoverable, isFalse);
  });

  test('polling refreshes every two seconds until stopped', () {
    fakeAsync((FakeAsync async) {
      final FakeP2pBridge bridge = FakeP2pBridge(status: emptyStatus());
      final DevicesController controller = DevicesController(p2pBridge: bridge);

      controller.startPolling();
      async.flushMicrotasks();
      async.elapse(const Duration(seconds: 4));
      final int whilePolling = bridge.statusRequestCount;
      controller.stopPolling();
      async.elapse(const Duration(seconds: 6));

      expect(whilePolling, 3, reason: 'one at start, then at 2 s and 4 s');
      expect(bridge.statusRequestCount, whilePolling);
      controller.dispose();
    });
  });

  test('discoverable ends after the five-minute pairing window', () {
    fakeAsync((FakeAsync async) {
      final DevicesController controller =
          DevicesController(p2pBridge: FakeP2pBridge(status: emptyStatus()));

      controller.makeDiscoverable();
      async.flushMicrotasks();
      expect(controller.discoverable, isTrue);
      async.elapse(const Duration(minutes: 5, seconds: 1));

      expect(controller.discoverable, isFalse);
      controller.dispose();
    });
  });
}
