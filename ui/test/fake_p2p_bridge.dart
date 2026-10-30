// Provides a scripted P2P bridge and status fixtures shared by the Devices tests.

import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';

/// One command received by the fake.
typedef P2pCommandCall = ({String command, String? peerId});

/// Serves a scripted status without loading Rust and records every command.
final class FakeP2pBridge implements P2pBridge {
  /// Creates a fake whose `getStatus` returns `status`.
  FakeP2pBridge({required this.status});

  /// The status the next `getStatus` returns; tests replace it to simulate progress.
  P2pStatus status;

  /// When set, `getStatus` throws this error.
  Object? statusError;

  /// When set, the next command throws this error once.
  Object? nextCommandError;

  /// The result commands return when they succeed.
  P2pCommandResult commandResult = const P2pCommandResult();

  /// Commands received, in order.
  final List<P2pCommandCall> commands = <P2pCommandCall>[];

  /// Transcription routes chosen, in order.
  final List<String> routes = <String>[];

  /// Number of `getStatus` calls.
  int statusRequestCount = 0;

  @override
  Future<P2pStatus> getStatus() async {
    statusRequestCount += 1;
    if (statusError case final Object error) {
      throw error;
    }
    return status;
  }

  @override
  Future<P2pCommandResult> runCommand(String command, {String? peerId}) async {
    commands.add((command: command, peerId: peerId));
    if (nextCommandError case final Object error) {
      nextCommandError = null;
      throw error;
    }
    return commandResult;
  }

  @override
  Future<void> setTranscriptionRoute(String route) async {
    routes.add(route);
    if (nextCommandError case final Object error) {
      nextCommandError = null;
      throw error;
    }
  }
}

/// A running status with one paired device, one nearby device, one request, and one pairing.
P2pStatus busyStatus() => P2pStatus(
      enabled: true,
      localPeerId: '12D3KooWLocal',
      deviceName: 'Studio laptop',
      capabilities: const DeviceCapabilities(
        cpuCores: 8,
        gpuVramMegabytes: 0,
        transcriptionModels: <String>['whisper'],
      ),
      pairedDevices: <PairedDevice>[
        PairedDevice(
          peerId: '12D3KooWPhone',
          deviceName: 'Galaxy',
          pairedAt: DateTime.utc(2026, 10, 28, 9, 52),
          capabilities: const DeviceCapabilities(
            cpuCores: 16,
            gpuVramMegabytes: 8192,
            transcriptionModels: <String>['whisper', 'openai'],
          ),
        ),
        PairedDevice(
          peerId: '12D3KooWOldTablet',
          deviceName: 'Old tablet',
          pairedAt: DateTime.utc(2026, 10, 27, 18),
        ),
      ],
      nearbyPeerIds: const <String>['12D3KooWNearbyDevice1234'],
      incomingRequests: const <IncomingPairingRequest>[
        IncomingPairingRequest(
            peerId: '12D3KooWTablet', deviceName: 'Tablet', code: '639210'),
      ],
      outgoingPairings: const <OutgoingPairing>[
        OutgoingPairing(
            peerId: '12D3KooWDesk', code: '888224', locallyConfirmed: false),
      ],
    );

/// A running status with nothing nearby or paired.
P2pStatus emptyStatus() => const P2pStatus(
      enabled: true,
      localPeerId: '12D3KooWLocal',
      deviceName: 'Studio laptop',
    );
