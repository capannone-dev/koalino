// Passes the app's data directory and options to the Rust core with `note_taker_configure`
// before any other native call (Issue #43).

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';

/// Peer addresses compiled in with `--dart-define=NOTE_TAKER_P2P_PEERS=<a>,<b>`, for development
/// builds that must reach devices mDNS cannot see (such as VMs behind an `adb reverse` tunnel).
/// Empty in normal builds; addresses are not secrets.
const String p2pPeersDefine = String.fromEnvironment('NOTE_TAKER_P2P_PEERS');

/// File name of the Whisper model the app looks for in `<data directory>/models`.
const String whisperModelFileName = 'ggml-base.bin';

/// Builds the JSON for `note_taker_configure`.
///
/// Inputs: The absolute app-private data directory, this device's name, and the model path when
/// the model is present. The keychain stays enabled everywhere; on Android it is the Keystore key
/// that `startCoreWithHostKey` hands over. `p2pPeersDefine` is a comma-separated list of
/// `…/p2p/<peer id>` addresses to dial; blank entries are dropped.
/// Outputs: The JSON object text; absent options are omitted, so Rust's defaults apply.
/// Side effects: None.
String appConfigurationJson({
  required String dataDirectory,
  required String deviceName,
  required String? whisperModelPath,
  String p2pPeersDefine = '',
}) {
  final List<String> peers = p2pPeersDefine
      .split(',')
      .map((String peer) => peer.trim())
      .where((String peer) => peer.isNotEmpty)
      .toList(growable: false);
  return jsonEncode(<String, Object>{
    'data_directory': dataDirectory,
    'device_name': deviceName,
    if (whisperModelPath != null) 'whisper_model_path': whisperModelPath,
    if (peers.isNotEmpty) 'p2p_peers': peers,
  });
}

/// Picks the name this device announces when pairing.
///
/// Outputs: The host name on desktops; on Android, whose host name is always `localhost`, a
/// generic name until the Settings screen lets the owner choose one.
/// Side effects: Reads the host name.
String defaultDeviceName() {
  if (Platform.isAndroid) {
    return 'Android device';
  }
  final String hostName = Platform.localHostname.trim();
  return hostName.isEmpty ? 'Note Taker device' : hostName;
}

/// Returns the app's data directory, `<application support>/note_taker`.
///
/// Side effects: Asks the platform for the application-support directory; creates nothing.
Future<Directory> appDataDirectory() async {
  final Directory supportDirectory = await getApplicationSupportDirectory();
  return Directory(
      '${supportDirectory.path}${Platform.pathSeparator}note_taker');
}

/// Returns the directory holding the encryption key material, `<data directory>/keys`.
///
/// Side effects: As `appDataDirectory`.
Future<String> appKeyDirectory() async =>
    '${(await appDataDirectory()).path}${Platform.pathSeparator}keys';

/// Configures the Rust core for this app; call once before any other bridge.
///
/// Inputs: None; the data directory is the platform's application-support directory.
/// Outputs: Completes with Rust's status: 0 on success, 1 for invalid settings, 2 when the core
/// was already configured or started.
/// Side effects: Creates `<support>/note_taker` and stores the settings in the native process.
Future<int> configureCore() async {
  final Directory dataDirectory = await appDataDirectory();
  await dataDirectory.create(recursive: true);
  final File model = File(
      '${dataDirectory.path}${Platform.pathSeparator}models${Platform.pathSeparator}$whisperModelFileName');
  final String configurationJson = appConfigurationJson(
    dataDirectory: dataDirectory.path,
    deviceName: defaultDeviceName(),
    whisperModelPath: await model.exists() ? model.path : null,
    p2pPeersDefine: p2pPeersDefine,
  );
  return Isolate.run(() {
    final DynamicLibrary coreLibrary = _openCoreLibrary();
    final _DartConfigure configure =
        coreLibrary.lookupFunction<_NativeConfigure, _DartConfigure>(
            'note_taker_configure');
    final Pointer<Utf8> nativeConfiguration = configurationJson.toNativeUtf8();
    try {
      return configure(nativeConfiguration);
    } finally {
      calloc.free(nativeConfiguration);
    }
  });
}

/// Opens the Rust runtime so device sync runs from app start, not from the first screen that
/// needs native data.
///
/// Outputs: Completes with Rust's status: 0 when the runtime is open, otherwise the failure,
/// which each screen then reports on its own.
/// Side effects: Opens the database and key store and starts P2P, on a background isolate.
Future<int> startCore() => Isolate.run(() => _openCoreLibrary()
    .lookupFunction<_NativeStartRuntime, _DartStartRuntime>(
        'note_taker_start_runtime')());

/// Opens the platform's core library.
DynamicLibrary _openCoreLibrary() => Platform.isWindows
    ? DynamicLibrary.open('note_taker_core.dll')
    : DynamicLibrary.open('libnote_taker_core.so');

/// Represents `int note_taker_start_runtime(void)`.
typedef _NativeStartRuntime = Int32 Function();

/// Represents the Dart callable form of `note_taker_start_runtime`.
typedef _DartStartRuntime = int Function();

/// Represents `int note_taker_configure(const char *configuration_json)`.
typedef _NativeConfigure = Int32 Function(Pointer<Utf8> configurationJson);

/// Represents the Dart callable form of `note_taker_configure`.
typedef _DartConfigure = int Function(Pointer<Utf8> configurationJson);
