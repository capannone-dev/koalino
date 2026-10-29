// Connects the Devices screen to the Rust P2P task through `note_taker_p2p_status` and
// `note_taker_p2p_command` (Issue #43).

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';

/// A device paired with this one.
final class PairedDevice {
  /// Creates a paired-device row.
  const PairedDevice({
    required this.peerId,
    required this.deviceName,
    required this.pairedAt,
    this.capabilities,
  });

  /// The device's libp2p peer ID, used to address commands.
  final String peerId;

  /// The name the device announced when it paired.
  final String deviceName;

  /// When the pairing completed, in UTC.
  final DateTime pairedAt;

  /// What the device advertised in its manifest; `null` while it is not connected.
  final DeviceCapabilities? capabilities;
}

/// What a device advertises in its manifest (REQUIREMENTS 3.4).
final class DeviceCapabilities {
  /// Creates a capabilities summary.
  const DeviceCapabilities({
    required this.cpuCores,
    required this.gpuVramMegabytes,
    required this.transcriptionModels,
  });

  /// CPU cores available to the app.
  final int cpuCores;

  /// GPU memory in megabytes; 0 when none is known.
  final int gpuVramMegabytes;

  /// Model labels such as `whisper` (on device) or `openai` (cloud).
  final List<String> transcriptionModels;
}

/// Another device asking to pair, awaiting the owner's answer.
final class IncomingPairingRequest {
  /// Creates a request row.
  const IncomingPairingRequest({
    required this.peerId,
    required this.deviceName,
    required this.code,
  });

  /// The requesting device's peer ID.
  final String peerId;

  /// The name the requesting device announced.
  final String deviceName;

  /// The six-digit code both devices show.
  final String code;
}

/// A pairing this device started.
final class OutgoingPairing {
  /// Creates a pairing row.
  const OutgoingPairing({
    required this.peerId,
    required this.code,
    required this.locallyConfirmed,
  });

  /// The other device's peer ID.
  final String peerId;

  /// The six-digit code both devices show.
  final String code;

  /// Whether the owner already confirmed matching codes here.
  final bool locallyConfirmed;
}

/// The P2P task's state as the Devices screen shows it.
final class P2pStatus {
  /// Creates a status; lists default to empty.
  const P2pStatus({
    required this.enabled,
    this.localPeerId = '',
    this.deviceName = '',
    this.capabilities,
    this.pairedDevices = const <PairedDevice>[],
    this.nearbyPeerIds = const <String>[],
    this.incomingRequests = const <IncomingPairingRequest>[],
    this.outgoingPairings = const <OutgoingPairing>[],
  });

  /// Whether the P2P task runs; every list is empty when it does not.
  final bool enabled;

  /// This device's peer ID.
  final String localPeerId;

  /// The name this device announces.
  final String deviceName;

  /// What this device advertises to its paired devices; `null` from an older core.
  final DeviceCapabilities? capabilities;

  /// Paired devices, oldest first.
  final List<PairedDevice> pairedDevices;

  /// Unpaired devices visible on the local network, by peer ID.
  final List<String> nearbyPeerIds;

  /// Requests awaiting an answer.
  final List<IncomingPairingRequest> incomingRequests;

  /// Pairings this device started that have not finished.
  final List<OutgoingPairing> outgoingPairings;
}

/// The successful answer to a P2P command.
final class P2pCommandResult {
  /// Creates a result; every field is optional.
  const P2pCommandResult({this.code, this.updateKind, this.revoked});

  /// The code shown by `start_pairing`.
  final String? code;

  /// The pairing update's `kind` after `confirm_code` or `accept_request`, such as `paired`.
  final String? updateKind;

  /// Whether `revoke` removed a paired device.
  final bool? revoked;
}

/// A refused P2P request, an unavailable task, or an unreadable answer.
final class P2pException implements Exception {
  /// Creates an exception with Rust's machine-readable `errorCode`.
  const P2pException(this.errorCode, this.message);

  /// `invalid_request`, `invalid_peer_id`, `unavailable`, `refused`, `runtime_unavailable`, or
  /// `malformed_response`.
  final String errorCode;

  /// Rust's explanation; it can contain paths, so widgets never show it.
  final String message;

  @override
  String toString() => 'P2pException($errorCode)';
}

/// Reads the P2P status and runs Devices-screen commands.
abstract interface class P2pBridge {
  /// Returns the latest status; throws `P2pException` when the runtime is unavailable.
  Future<P2pStatus> getStatus();

  /// Runs `command` (such as `start_pairing`) for `peerId` when the command needs one.
  ///
  /// Throws `P2pException` with Rust's error code when the command is refused.
  Future<P2pCommandResult> runCommand(String command, {String? peerId});
}

/// Builds the JSON request for `note_taker_p2p_command`.
///
/// Inputs: The command name and, for every command but `make_pairable`, the peer ID.
/// Outputs: `{"command":…}` or `{"command":…,"peer_id":…}`.
/// Side effects: None.
String p2pCommandJson(String command, {String? peerId}) =>
    jsonEncode(<String, String>{
      'command': command,
      if (peerId != null) 'peer_id': peerId,
    });

/// Decodes `note_taker_p2p_status` JSON.
///
/// Inputs: The JSON text from Rust.
/// Outputs: The status.
/// Side effects: None. Throws `P2pException` for an error object or malformed JSON.
P2pStatus decodeP2pStatus(String json) {
  final Map<String, Object?> object = _decodeObject(json);
  if (object['ok'] == false) {
    throw _exceptionFrom(object);
  }
  if (object['enabled'] != true) {
    return const P2pStatus(enabled: false);
  }
  try {
    return P2pStatus(
      enabled: true,
      localPeerId: object['local_peer_id']! as String,
      deviceName: object['device_name']! as String,
      capabilities: _capabilities(object['capabilities']),
      pairedDevices: _list(object['trusted_devices'])
          .map((Map<String, Object?> device) => PairedDevice(
                peerId: device['peer_id']! as String,
                deviceName: device['device_name']! as String,
                pairedAt: DateTime.fromMillisecondsSinceEpoch(
                    (device['paired_at_ms']! as num).toInt(),
                    isUtc: true),
                capabilities: _capabilities(device['capabilities']),
              ))
          .toList(growable: false),
      nearbyPeerIds: _list(object['nearby_devices'])
          .map((Map<String, Object?> device) => device['peer_id']! as String)
          .toList(growable: false),
      incomingRequests: _list(object['incoming_requests'])
          .map((Map<String, Object?> request) => IncomingPairingRequest(
                peerId: request['peer_id']! as String,
                deviceName: request['device_name']! as String,
                code: request['code']! as String,
              ))
          .toList(growable: false),
      outgoingPairings: _list(object['outgoing_pairings'])
          .map((Map<String, Object?> pairing) => OutgoingPairing(
                peerId: pairing['peer_id']! as String,
                code: pairing['code']! as String,
                locallyConfirmed: pairing['locally_confirmed']! as bool,
              ))
          .toList(growable: false),
    );
  } on TypeError {
    throw const P2pException(
        'malformed_response', 'the P2P status has an unexpected shape');
  }
}

/// Decodes a `note_taker_p2p_command` answer.
///
/// Inputs: The JSON text from Rust.
/// Outputs: The result of a successful command.
/// Side effects: None. Throws `P2pException` for `{"ok":false,…}` or malformed JSON.
P2pCommandResult decodeP2pCommandResult(String json) {
  final Map<String, Object?> object = _decodeObject(json);
  if (object['ok'] != true) {
    throw _exceptionFrom(object);
  }
  final Object? update = object['update'];
  return P2pCommandResult(
    code: object['code'] as String?,
    updateKind:
        update is Map<String, Object?> ? update['kind'] as String? : null,
    revoked: object['revoked'] as bool?,
  );
}

/// Decodes a capabilities object, or `null` when it is absent or `null`.
///
/// Throws `TypeError` for an object of the wrong shape (reported as `malformed_response`).
DeviceCapabilities? _capabilities(Object? value) {
  if (value == null) {
    return null;
  }
  final Map<String, Object?> object = value as Map<String, Object?>;
  return DeviceCapabilities(
    cpuCores: (object['cpu_cores']! as num).toInt(),
    gpuVramMegabytes: (object['gpu_vram_megabytes']! as num).toInt(),
    transcriptionModels: (object['transcription_models']! as List<Object?>)
        .map((Object? model) => model! as String)
        .toList(growable: false),
  );
}

/// Parses `json` as an object or throws `P2pException`.
Map<String, Object?> _decodeObject(String json) {
  try {
    final Object? decoded = jsonDecode(json);
    if (decoded is Map<String, Object?>) {
      return decoded;
    }
  } on FormatException {
    // Reported below.
  }
  throw const P2pException(
      'malformed_response', 'the answer is not a JSON object');
}

/// Reads a list of objects, treating a missing field as empty.
List<Map<String, Object?>> _list(Object? value) => value == null
    ? const <Map<String, Object?>>[]
    : (value as List<Object?>).cast<Map<String, Object?>>();

/// Builds the exception for an `{"ok":false,…}` object.
P2pException _exceptionFrom(Map<String, Object?> object) => P2pException(
      object['error_code'] as String? ?? 'malformed_response',
      object['error'] as String? ?? '',
    );

/// Calls the Rust P2P exports, each on a background isolate.
final class RustP2pBridge implements P2pBridge {
  /// Creates the bridge; the library is opened inside each isolate.
  const RustP2pBridge();

  @override
  Future<P2pStatus> getStatus() async => decodeP2pStatus(await _fetchStatus());

  @override
  Future<P2pCommandResult> runCommand(String command, {String? peerId}) async =>
      decodeP2pCommandResult(
          await _runCommand(p2pCommandJson(command, peerId: peerId)));
}

/// Calls `note_taker_p2p_status` on a background isolate.
Future<String> _fetchStatus() => Isolate.run(() => _callNativeJson(
      (DynamicLibrary coreLibrary) => coreLibrary.lookupFunction<
          _NativeNoArgumentJson,
          _DartNoArgumentJson>('note_taker_p2p_status')(),
    ));

/// Calls `note_taker_p2p_command` on a background isolate; it blocks until the task answers.
///
/// Declared at top level so the isolate closure captures only the request string.
Future<String> _runCommand(String requestJson) =>
    Isolate.run(() => _callNativeJson((DynamicLibrary coreLibrary) {
          final Pointer<Utf8> nativeRequest = requestJson.toNativeUtf8();
          try {
            return coreLibrary.lookupFunction<_NativeOneStringJson,
                _DartOneStringJson>('note_taker_p2p_command')(nativeRequest);
          } finally {
            calloc.free(nativeRequest);
          }
        }));

/// Runs a caller-freed JSON call against the core library and copies its answer out.
String _callNativeJson(
    Pointer<Utf8> Function(DynamicLibrary coreLibrary) invokeCommand) {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final _DartFreeString freeString =
      coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
          'note_taker_free_string');
  final Pointer<Utf8> responsePointer = invokeCommand(coreLibrary);
  if (responsePointer == nullptr) {
    throw const P2pException('malformed_response', 'Rust returned no answer');
  }
  try {
    return responsePointer.toDartString();
  } finally {
    freeString(responsePointer);
  }
}

/// Opens the platform-specific core library.
DynamicLibrary _openCoreLibrary() {
  if (Platform.isAndroid || Platform.isLinux) {
    return DynamicLibrary.open('libnote_taker_core.so');
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('note_taker_core.dll');
  }
  throw UnsupportedError('The Rust P2P bridge is unavailable here.');
}

/// Represents `char *note_taker_p2p_status(void)`.
typedef _NativeNoArgumentJson = Pointer<Utf8> Function();

/// Represents the Dart callable form of `note_taker_p2p_status`.
typedef _DartNoArgumentJson = Pointer<Utf8> Function();

/// Represents `char *note_taker_p2p_command(const char *request_json)`.
typedef _NativeOneStringJson = Pointer<Utf8> Function(Pointer<Utf8> argument);

/// Represents the Dart callable form of `note_taker_p2p_command`.
typedef _DartOneStringJson = Pointer<Utf8> Function(Pointer<Utf8> argument);

/// Represents `void note_taker_free_string(char *value)`.
typedef _NativeFreeString = Void Function(Pointer<Utf8> value);

/// Represents the Dart callable form of `note_taker_free_string`.
typedef _DartFreeString = void Function(Pointer<Utf8> value);
