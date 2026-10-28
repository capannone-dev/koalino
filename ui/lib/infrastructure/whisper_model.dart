// Finds the Whisper model the Rust core transcribes with, so a fresh install works out of the
// box and offline. A model the owner placed in `<data directory>/models` wins. Otherwise Linux
// and Windows use the copy their bundle ships in `data/models` beside the executable, and
// Android copies the one shipped in its APK into the data directory (`WhisperModel.kt`, channel
// `note_taker/whisper_model`), because whisper.cpp reads the model from a file path.

import 'dart:io';

import 'package:flutter/services.dart' show MethodChannel, PlatformException;

/// File name of the shipped Whisper model (multilingual base).
const String whisperModelFileName = 'ggml-base.bin';

/// The platform channel `MainActivity` registers for the shipped model.
const String whisperModelChannelName = 'note_taker/whisper_model';

/// Copies the shipped model to `targetPath` when needed; true once a model is there.
typedef AndroidModelInstaller = Future<bool> Function(String targetPath);

/// Returns where a Linux or Windows bundle ships the model.
///
/// Inputs: The running executable's absolute path (`Platform.resolvedExecutable`); its own
/// separator style is kept, so Windows paths work on any host.
/// Outputs: `<executable directory>/data/models/ggml-base.bin`.
/// Side effects: None.
String bundledDesktopModelPath(String executablePath) {
  final String separator = executablePath.contains(r'\') ? r'\' : '/';
  final int lastSeparator = executablePath.lastIndexOf(separator);
  final String bundleDirectory =
      lastSeparator < 0 ? '.' : executablePath.substring(0, lastSeparator);
  return <String>[bundleDirectory, 'data', 'models', whisperModelFileName]
      .join(separator);
}

/// Picks the model path passed to `note_taker_configure`.
///
/// Inputs: The app's data directory, the running executable's path, whether this is Android,
/// and the Android installer (called only on Android).
/// Outputs: The model's absolute path, or `null` when there is none (the core then reports the
/// pipeline as unavailable when a recording starts).
/// Side effects: Checks files; on Android may copy the shipped model (about 150 MB, first start
/// or after an update that ships a different model).
Future<String?> resolveWhisperModelPath({
  required String dataDirectory,
  required String executablePath,
  required bool isAndroid,
  required AndroidModelInstaller installAndroidModel,
}) async {
  final String separator = Platform.pathSeparator;
  final String dataModelPath =
      '$dataDirectory${separator}models$separator$whisperModelFileName';
  if (isAndroid) {
    return await installAndroidModel(dataModelPath) ? dataModelPath : null;
  }
  if (await File(dataModelPath).exists()) {
    return dataModelPath;
  }
  final String bundledPath = bundledDesktopModelPath(executablePath);
  return await File(bundledPath).exists() ? bundledPath : null;
}

/// Installs the APK's model over the `note_taker/whisper_model` channel.
///
/// Inputs: The target path in the app's data directory.
/// Outputs: true when a model is at the target afterwards; false when the APK ships none or the
/// copy failed (the channel's error is not rethrown: the app still starts, without a model).
/// Side effects: Kotlin copies the asset on a background thread when the target is missing or
/// differs in size.
Future<bool> installAndroidWhisperModel(String targetPath) async {
  const MethodChannel channel = MethodChannel(whisperModelChannelName);
  try {
    return await channel.invokeMethod<bool>(
            'install', <String, String>{'targetPath': targetPath}) ??
        false;
  } on PlatformException {
    return false;
  }
}
