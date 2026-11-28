// Asks the Android host to download what Whisper needs on a Snapdragon NPU (#75), over the
// `note_taker/npu` channel served by `NpuDownload.kt`. The host fetches Qualcomm's QNN runtime and
// the precompiled Whisper Small, checks their SHA-256 and installs them in the core's NPU folder.
// Other platforms have no NPU engine and use `NoNpuDownloader`.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';

/// Where a download stands.
final class NpuDownloadProgress {
  /// Creates the progress.
  const NpuDownloadProgress({
    required this.state,
    this.receivedBytes = 0,
    this.totalBytes = 0,
    this.error,
  });

  /// Reads the host's answer; anything unexpected counts as idle.
  factory NpuDownloadProgress.fromMap(Map<Object?, Object?> map) =>
      NpuDownloadProgress(
        state: map['state'] as String? ?? 'idle',
        receivedBytes: map['receivedBytes'] as int? ?? 0,
        totalBytes: map['totalBytes'] as int? ?? 0,
        error: map['error'] as String?,
      );

  /// `idle`, `downloading`, `done`, `cancelled` or `failed`.
  final String state;

  /// Bytes received over both archives.
  final int receivedBytes;

  /// Both archives' size.
  final int totalBytes;

  /// Why it failed: `no_network`, `server`, `corrupted` or `storage`.
  final String? error;

  /// The share received, from 0 to 1, or `null` before the size is known.
  double? get fraction =>
      totalBytes > 0 ? (receivedBytes / totalBytes).clamp(0.0, 1.0) : null;
}

/// Downloads and removes the NPU files.
abstract interface class NpuDownloader {
  /// Starts downloading `build` and its QNN libraries into `directory` (the core's NPU folder).
  Future<void> download(String directory, NpuBuild build);

  /// Reports the running or latest download.
  Future<NpuDownloadProgress> progress();

  /// Stops the running download; nothing partial is kept.
  Future<void> cancel();

  /// Deletes the downloaded files; answers whether they are gone.
  Future<bool> remove(String directory);
}

/// A platform without the NPU engine.
final class NoNpuDownloader implements NpuDownloader {
  /// Creates the downloader.
  const NoNpuDownloader();

  @override
  Future<void> download(String directory, NpuBuild build) async {}

  @override
  Future<NpuDownloadProgress> progress() async =>
      const NpuDownloadProgress(state: 'failed', error: 'unsupported');

  @override
  Future<void> cancel() async {}

  @override
  Future<bool> remove(String directory) async => false;
}

/// The Android host's downloader.
final class ChannelNpuDownloader implements NpuDownloader {
  /// Creates the downloader.
  const ChannelNpuDownloader();

  static const MethodChannel _channel = MethodChannel('note_taker/npu');

  /// Side effects: The host downloads about 360 MB in the background.
  @override
  Future<void> download(String directory, NpuBuild build) =>
      _channel.invokeMethod<void>('download', <String, Object?>{
        'directory': directory,
        'modelUrl': build.url,
        'modelSha256': build.sha256,
        'modelSizeBytes': build.sizeBytes,
        'modelEntryPrefix': build.entryPrefix,
        'hexagon': build.hexagon,
      });

  @override
  Future<NpuDownloadProgress> progress() async {
    final Map<Object?, Object?>? answer =
        await _channel.invokeMethod<Map<Object?, Object?>>('progress');
    return answer == null
        ? const NpuDownloadProgress(state: 'idle')
        : NpuDownloadProgress.fromMap(answer);
  }

  @override
  Future<void> cancel() => _channel.invokeMethod<void>('cancel');

  /// Side effects: Deletes the NPU folder.
  @override
  Future<bool> remove(String directory) async =>
      await _channel.invokeMethod<bool>(
          'remove', <String, Object?>{'directory': directory}) ??
      false;
}

/// The downloader for this platform.
NpuDownloader platformNpuDownloader() =>
    Platform.isAndroid ? const ChannelNpuDownloader() : const NoNpuDownloader();
