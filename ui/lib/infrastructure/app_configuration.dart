// Passes the app's data directory and options to the Rust core with `note_taker_configure`
// before any other native call (Issue #43).

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:ffi/ffi.dart';
import 'package:path_provider/path_provider.dart';

/// File name of the Whisper model the app looks for in `<data directory>/models`.
const String whisperModelFileName = 'ggml-base.bin';

/// Builds the JSON for `note_taker_configure`.
///
/// Inputs: The absolute app-private data directory, this device's name, the model path when the
/// model is present, and whether the platform is Android (which has no keychain backend, so the
/// key file stays in the app-private directory).
/// Outputs: The JSON object text; absent options are omitted, so Rust's defaults apply.
/// Side effects: None.
String appConfigurationJson({
  required String dataDirectory,
  required String deviceName,
  required String? whisperModelPath,
  required bool isAndroid,
}) =>
    jsonEncode(<String, Object>{
      'data_directory': dataDirectory,
      'device_name': deviceName,
      if (whisperModelPath != null) 'whisper_model_path': whisperModelPath,
      if (isAndroid) 'keychain_enabled': false,
    });

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

/// Configures the Rust core for this app; call once before any other bridge.
///
/// Inputs: None; the data directory is the platform's application-support directory.
/// Outputs: Completes with Rust's status: 0 on success, 1 for invalid settings, 2 when the core
/// was already configured or started.
/// Side effects: Creates `<support>/note_taker` and stores the settings in the native process.
Future<int> configureCore() async {
  final Directory supportDirectory = await getApplicationSupportDirectory();
  final Directory dataDirectory =
      Directory('${supportDirectory.path}${Platform.pathSeparator}note_taker');
  await dataDirectory.create(recursive: true);
  final File model = File(
      '${dataDirectory.path}${Platform.pathSeparator}models${Platform.pathSeparator}$whisperModelFileName');
  final String configurationJson = appConfigurationJson(
    dataDirectory: dataDirectory.path,
    deviceName: defaultDeviceName(),
    whisperModelPath: await model.exists() ? model.path : null,
    isAndroid: Platform.isAndroid,
  );
  return Isolate.run(() {
    final DynamicLibrary coreLibrary = Platform.isWindows
        ? DynamicLibrary.open('note_taker_core.dll')
        : DynamicLibrary.open('libnote_taker_core.so');
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

/// Represents `int note_taker_configure(const char *configuration_json)`.
typedef _NativeConfigure = Int32 Function(Pointer<Utf8> configurationJson);

/// Represents the Dart callable form of `note_taker_configure`.
typedef _DartConfigure = int Function(Pointer<Utf8> configurationJson);
