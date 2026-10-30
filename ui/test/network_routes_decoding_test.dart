// Specifies decoding the core's network routes and invite answers.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';

void main() {
  test('decodes the routes in the status, and their absence from an older core',
      () {
    final P2pStatus status =
        decodeP2pStatus('{"enabled":true,"local_peer_id":"a","device_name":"b",'
            '"network":{"relay_address":"/ip4/1.2.3.4/tcp/1/p2p/r",'
            '"relay_state":"reserved","peers":["/ip4/1.2.3.5/tcp/1/p2p/s"],'
            '"offers_relay":false}}');
    final P2pStatus older = decodeP2pStatus(
        '{"enabled":true,"local_peer_id":"a","device_name":"b"}');

    expect(status.network.relayAddress, '/ip4/1.2.3.4/tcp/1/p2p/r');
    expect(status.network.relayState, 'reserved');
    expect(status.network.peers, <String>['/ip4/1.2.3.5/tcp/1/p2p/s']);
    expect(status.network.hasRelay, isTrue);
    expect(older.network.hasRelay, isFalse);
  });

  test('decodes an invite and a joined device', () {
    final PairingInviteLink invite = decodePairingInvite(
        '{"ok":true,"invite":"note-taker-invite:1;a;5;/x","expires_at_ms":5000}');

    expect(invite.text, 'note-taker-invite:1;a;5;/x');
    expect(invite.expiresAt.millisecondsSinceEpoch, 5000);
    expect(decodeJoinedInvite('{"ok":true,"peer_id":"12D3"}'), '12D3');
  });

  test('turns a refused invite into its code', () {
    expect(
      () => decodeJoinedInvite(
          '{"ok":false,"error_code":"invite_expired","error":"x"}'),
      throwsA(isA<P2pException>().having(
          (P2pException error) => error.errorCode, 'code', 'invite_expired')),
    );
  });
}
