// Specifies how the Devices screen decodes the P2P JSON from Rust and builds the configure JSON.

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/app_configuration.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';

/// A status as `note_taker_p2p_status` returns it, with one item of each kind.
const String runningStatusJson = '''
{"enabled":true,"local_peer_id":"12D3KooWLocal","device_name":"Studio laptop",
 "listen_addresses":["/ip4/192.168.1.5/tcp/4001"],
 "capabilities":{"cpu_cores":8,"gpu_vram_megabytes":4096,"transcription_models":["whisper","openai"],"supports_vad":true},
 "trusted_devices":[{"peer_id":"12D3KooWPhone","device_name":"Galaxy","paired_at_ms":1793181121148,"capabilities":null}],
 "nearby_devices":[{"peer_id":"12D3KooWNearby"}],
 "incoming_requests":[{"peer_id":"12D3KooWTablet","device_name":"Tablet","code":"639210"}],
 "outgoing_pairings":[{"peer_id":"12D3KooWDesk","code":"888224","locally_confirmed":true}],
 "recent_updates":[{"kind":"paired","peer_id":"12D3KooWPhone","device_name":"Galaxy"}]}
''';

void main() {
  group('decodeP2pStatus', () {
    test('a running task lists every kind of device', () {
      final P2pStatus status = decodeP2pStatus(runningStatusJson);

      expect(status.enabled, isTrue);
      expect(status.localPeerId, '12D3KooWLocal');
      expect(status.deviceName, 'Studio laptop');
      expect(status.pairedDevices.single.peerId, '12D3KooWPhone');
      expect(status.pairedDevices.single.deviceName, 'Galaxy');
      expect(
        status.pairedDevices.single.pairedAt,
        DateTime.fromMillisecondsSinceEpoch(1793181121148, isUtc: true),
      );
      expect(status.capabilities?.cpuCores, 8);
      expect(status.capabilities?.gpuVramMegabytes, 4096);
      expect(status.capabilities?.transcriptionModels,
          <String>['whisper', 'openai']);
      expect(status.pairedDevices.single.capabilities, isNull);
      expect(status.nearbyPeerIds, <String>['12D3KooWNearby']);
      expect(status.incomingRequests.single.deviceName, 'Tablet');
      expect(status.incomingRequests.single.code, '639210');
      expect(status.outgoingPairings.single.code, '888224');
      expect(status.outgoingPairings.single.locallyConfirmed, isTrue);
    });

    test('a disabled task has no devices', () {
      final P2pStatus status = decodeP2pStatus('{"enabled":false}');

      expect(status.enabled, isFalse);
      expect(status.pairedDevices, isEmpty);
      expect(status.nearbyPeerIds, isEmpty);
    });

    test('an error object or malformed JSON is a P2pException', () {
      expect(
        () => decodeP2pStatus(
            '{"ok":false,"error_code":"runtime_unavailable","error":"x"}'),
        throwsA(isA<P2pException>().having(
            (P2pException error) => error.errorCode,
            'errorCode',
            'runtime_unavailable')),
      );
      expect(() => decodeP2pStatus('[1]'), throwsA(isA<P2pException>()));
    });
  });

  group('decodeP2pCommandResult', () {
    test('successes carry the code, the update kind, or the revocation', () {
      expect(
          decodeP2pCommandResult('{"ok":true,"code":"123456"}').code, '123456');
      expect(
          decodeP2pCommandResult(
                  '{"ok":true,"update":{"kind":"paired","peer_id":"A","device_name":"B"}}')
              .updateKind,
          'paired');
      expect(decodeP2pCommandResult('{"ok":true,"revoked":false}').revoked,
          isFalse);
    });

    test('a failure is a P2pException with its error code', () {
      expect(
        () => decodeP2pCommandResult(
            '{"ok":false,"error_code":"refused","error":"no pairing"}'),
        throwsA(isA<P2pException>().having(
            (P2pException error) => error.errorCode, 'errorCode', 'refused')),
      );
    });
  });

  test('p2pCommandJson tags the command and adds the peer ID only when given',
      () {
    expect(jsonDecode(p2pCommandJson('make_pairable')),
        <String, Object?>{'command': 'make_pairable'});
    expect(jsonDecode(p2pCommandJson('revoke', peerId: '12D3KooWA')),
        <String, Object?>{'command': 'revoke', 'peer_id': '12D3KooWA'});
  });

  group('appConfigurationJson', () {
    test('every platform uses its keychain; Android through the Keystore', () {
      final Map<String, Object?> configuration =
          jsonDecode(appConfigurationJson(
        dataDirectory: '/data/user/0/app/files/note_taker',
        deviceName: 'Galaxy S23',
        whisperModelPath:
            '/data/user/0/app/files/note_taker/models/ggml-base.bin',
      )) as Map<String, Object?>;

      expect(configuration, <String, Object?>{
        'data_directory': '/data/user/0/app/files/note_taker',
        'device_name': 'Galaxy S23',
        'whisper_model_path':
            '/data/user/0/app/files/note_taker/models/ggml-base.bin',
      });
    });

    test('desktops use the keychain and omit a missing model', () {
      final Map<String, Object?> configuration =
          jsonDecode(appConfigurationJson(
        dataDirectory: '/home/user/.local/share/note_taker',
        deviceName: 'laptop',
        whisperModelPath: null,
      )) as Map<String, Object?>;

      expect(configuration, <String, Object?>{
        'data_directory': '/home/user/.local/share/note_taker',
        'device_name': 'laptop',
      });
    });

    test(
        'a build-time relay address is passed trimmed, and left out when blank',
        () {
      Map<String, Object?> configurationWith(String relay) =>
          jsonDecode(appConfigurationJson(
            dataDirectory: '/data/note_taker',
            deviceName: 'Android device',
            whisperModelPath: null,
            p2pRelayDefine: relay,
          )) as Map<String, Object?>;

      expect(
          configurationWith(
              ' /ip4/203.0.113.5/tcp/4001/p2p/12D3KooWR ')['p2p_relay'],
          '/ip4/203.0.113.5/tcp/4001/p2p/12D3KooWR');
      expect(configurationWith('  ').containsKey('p2p_relay'), isFalse);
    });

    test('build-time peer addresses are passed as a list, blanks dropped', () {
      final Map<String, Object?> configuration =
          jsonDecode(appConfigurationJson(
        dataDirectory: '/data/note_taker',
        deviceName: 'Android device',
        whisperModelPath: null,
        p2pPeersDefine:
            ' /ip4/127.0.0.1/tcp/14001/p2p/12D3KooWA ,,/ip4/10.0.0.2/tcp/4001/p2p/12D3KooWB',
      )) as Map<String, Object?>;

      expect(configuration['p2p_peers'], <String>[
        '/ip4/127.0.0.1/tcp/14001/p2p/12D3KooWA',
        '/ip4/10.0.0.2/tcp/4001/p2p/12D3KooWB',
      ]);
    });
  });
}
