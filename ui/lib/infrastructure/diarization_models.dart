// Finds the diarization models the app ships, so a fresh install tells speakers apart offline.
// Linux and Windows bundles carry them in `data/models/diarization` beside the executable, whose
// path is passed to the Rust core. Android copies them out of its APK into the data directory
// (`WhisperModel.kt`), where the core looks by itself, as it does for models the owner put there.

import 'dart:io';

import 'package:flutter/services.dart' show MethodChannel, PlatformException;

import 'package:note_taker_ui/infrastructure/whisper_model.dart';

/// The two model files the core loads.
const List<String> diarizationModelFileNames = <String>[
  'segmentation.onnx',
  'embedding.onnx',
];

/// Copies a shipped asset to a target path when needed; true once a model is there.
typedef AndroidAssetInstaller = Future<bool> Function(
    String assetPath, String targetPath);

/// Returns where a desktop bundle ships the models.
///
/// Inputs: The running executable's absolute path; its separator style is kept.
/// Outputs: `diarization` in `bundledModelsDirectory`: `<executable directory>/data/models` on
/// Linux and Windows, the app's Contents/Resources/models on macOS (#151).
/// Side effects: None.
String bundledDiarizationModelDirectory(String executablePath) {
  final String separator = executablePath.contains(r'\') ? r'\' : '/';
  return '${bundledModelsDirectory(executablePath)}${separator}diarization';
}

/// Picks the folder passed to `note_taker_configure` as `diarization_model_directory`.
///
/// Inputs: The app's data directory, the running executable's path, whether this is Android, and
/// the Android installer (called only on Android, once per model).
/// Outputs: The bundle's folder when it has both models; `null` otherwise, and always on Android,
/// whose models end up in the data directory. Without models recordings get no speaker turns.
/// Side effects: Checks files; on Android may copy about 90 MB (first start or after an update
/// that ships different models).
Future<String?> resolveDiarizationModelDirectory({
  required String dataDirectory,
  required String executablePath,
  required bool isAndroid,
  required AndroidAssetInstaller installAndroidModel,
}) async {
  if (isAndroid) {
    for (final String fileName in diarizationModelFileNames) {
      await installAndroidModel(
        'models/diarization/$fileName',
        '$dataDirectory/models/diarization/$fileName',
      );
    }
    return null;
  }
  final String directory = bundledDiarizationModelDirectory(executablePath);
  final String separator = directory.contains(r'\') ? r'\' : '/';
  for (final String fileName in diarizationModelFileNames) {
    if (!await File('$directory$separator$fileName').exists()) {
      return null;
    }
  }
  return directory;
}

/// Installs one model from the APK over the `note_taker/whisper_model` channel.
///
/// Inputs: The asset's path in the APK and the target path in the app's data directory.
/// Outputs: true when a model is at the target afterwards; false when the APK ships none or the
/// copy failed (the channel's error is not rethrown: the app still starts, without diarization).
/// Side effects: Kotlin copies the asset on a background thread when needed.
Future<bool> installAndroidDiarizationModel(
    String assetPath, String targetPath) async {
  const MethodChannel channel = MethodChannel(whisperModelChannelName);
  try {
    return await channel.invokeMethod<bool>('install', <String, String>{
          'targetPath': targetPath,
          'assetPath': assetPath,
        }) ??
        false;
  } on PlatformException {
    return false;
  }
}
