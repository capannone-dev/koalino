// Defines the Dart-native boundary for Voice Gallery metadata and local playback.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';

import 'package:audioplayers/audioplayers.dart';
import 'package:ffi/ffi.dart';

/// Identifies one user-visible speaker identity without exposing biometric vectors.
final class VoiceIdentity {
  /// Creates immutable identity metadata returned by the application boundary.
  const VoiceIdentity({required this.identityId, required this.displayName});

  /// Stable identity UUID used by confirmation requests.
  final String identityId;

  /// Privacy-safe name shown in the Voice Gallery.
  final String displayName;
}

/// Describes an uncertain voice-match candidate retained with a review clip.
final class WeakMatch {
  /// Creates immutable weak-match metadata.
  const WeakMatch({required this.identityId, required this.confidenceScore});

  /// Suggested identity UUID.
  final String identityId;

  /// Adapter-provided similarity score in the inclusive 0-to-1 range.
  final double confidenceScore;
}

/// Describes one locally playable review clip returned as FFI JSON metadata.
final class VoiceClip {
  /// Creates an immutable clip list item.
  const VoiceClip({
    required this.clipId,
    required this.startTimeMs,
    required this.endTimeMs,
    required this.localFilePath,
    this.weakMatch,
  });

  /// Stable clip UUID.
  final String clipId;

  /// Segment start relative to the meeting recording.
  final int startTimeMs;

  /// Segment end relative to the meeting recording.
  final int endTimeMs;

  /// Absolute native file path. Audio bytes never cross the FFI boundary.
  final String localFilePath;

  /// Candidate requiring user confirmation, if the diarizer found one.
  final WeakMatch? weakMatch;

  /// Decodes the documented FFI JSON shape without accepting audio payloads.
  factory VoiceClip.fromJson(Map<String, Object?> json) {
    final Object? weakMatchValue = json['weak_match'];
    final Map<String, Object?>? weakMatchJson =
        weakMatchValue is Map ? weakMatchValue.cast<String, Object?>() : null;
    return VoiceClip(
      clipId: json['clip_id']! as String,
      startTimeMs: json['start_time_ms']! as int,
      endTimeMs: json['end_time_ms']! as int,
      localFilePath: json['local_file_path']! as String,
      weakMatch: weakMatchJson == null
          ? null
          : WeakMatch(
              identityId: weakMatchJson['identity_id']! as String,
              confidenceScore:
                  (weakMatchJson['confidence_score']! as num).toDouble(),
            ),
    );
  }
}

/// Declares Voice Gallery operations available to presentation and controller code.
abstract interface class VoiceGalleryBridge {
  /// Loads local-file metadata for unresolved clips belonging to `meetingId`.
  Future<List<VoiceClip>> getUnassignedClips(String meetingId);

  /// Lists identities that may be selected for a clip assignment.
  Future<List<VoiceIdentity>> getVoiceIdentities();

  /// Delegates file playback to the platform audio package.
  Future<void> playLocalFile(String localFilePath);

  /// Confirms a candidate identity for a weakly matched clip.
  Future<void> confirmWeakMatch(
      {required String clipId, required String identityId});

  /// Creates and assigns a named identity from an unresolved clip.
  Future<void> assignNewIdentity({
    required String clipId,
    required String displayName,
  });
}

/// Implements Voice Gallery metadata operations with C ABI calls and native file playback.
final class RustVoiceGalleryBridge implements VoiceGalleryBridge {
  /// Resolves C symbols from `dynamicLibrary` or the platform core library.
  RustVoiceGalleryBridge(
      {DynamicLibrary? dynamicLibrary, AudioPlayer? audioPlayer})
      : _dynamicLibrary = dynamicLibrary ?? _openCoreLibrary(),
        _audioPlayer = audioPlayer ?? AudioPlayer() {
    _getUnassignedClips = _dynamicLibrary.lookupFunction<
        _NativeGetUnassignedClips,
        _DartGetUnassignedClips>('note_taker_get_unassigned_clips');
    _confirmWeakMatch = _dynamicLibrary.lookupFunction<_NativeConfirmWeakMatch,
        _DartConfirmWeakMatch>('note_taker_confirm_weak_match');
    _assignNewIdentity = _dynamicLibrary.lookupFunction<
        _NativeAssignNewIdentity,
        _DartAssignNewIdentity>('note_taker_assign_new_identity');
    _freeString =
        _dynamicLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
      'note_taker_free_string',
    );
  }

  final DynamicLibrary _dynamicLibrary;
  final AudioPlayer _audioPlayer;
  late final _DartGetUnassignedClips _getUnassignedClips;
  late final _DartConfirmWeakMatch _confirmWeakMatch;
  late final _DartAssignNewIdentity _assignNewIdentity;
  late final _DartFreeString _freeString;

  @override
  Future<List<VoiceClip>> getUnassignedClips(String meetingId) async {
    final Pointer<Utf8> nativeMeetingId = meetingId.toNativeUtf8();
    try {
      final Pointer<Utf8> responsePointer =
          _getUnassignedClips(nativeMeetingId);
      if (responsePointer == nullptr) {
        throw StateError('Rust could not load unassigned voice clips.');
      }
      try {
        final Object? decodedResponse =
            jsonDecode(responsePointer.toDartString());
        if (decodedResponse is! List) {
          throw const FormatException(
              'Voice clip response must be a JSON array.');
        }
        return decodedResponse
            .cast<Map<Object?, Object?>>()
            .map((Map<Object?, Object?> json) =>
                VoiceClip.fromJson(json.cast<String, Object?>()))
            .toList(growable: false);
      } finally {
        _freeString(responsePointer);
      }
    } finally {
      calloc.free(nativeMeetingId);
    }
  }

  @override
  Future<List<VoiceIdentity>> getVoiceIdentities() async =>
      const <VoiceIdentity>[];

  @override
  Future<void> playLocalFile(String localFilePath) async {
    await _audioPlayer.play(DeviceFileSource(localFilePath));
  }

  @override
  Future<void> confirmWeakMatch({
    required String clipId,
    required String identityId,
  }) =>
      _sendTwoStrings(_confirmWeakMatch, clipId, identityId);

  @override
  Future<void> assignNewIdentity({
    required String clipId,
    required String displayName,
  }) =>
      _sendTwoStrings(_assignNewIdentity, clipId, displayName);

  /// Invokes a status-returning two-string C command and releases temporary buffers.
  Future<void> _sendTwoStrings(
    _DartTwoStringCommand command,
    String firstValue,
    String secondValue,
  ) async {
    final Pointer<Utf8> nativeFirstValue = firstValue.toNativeUtf8();
    final Pointer<Utf8> nativeSecondValue = secondValue.toNativeUtf8();
    try {
      if (command(nativeFirstValue, nativeSecondValue) != _ffiSuccess) {
        throw StateError('Rust rejected the Voice Gallery request.');
      }
    } finally {
      calloc.free(nativeFirstValue);
      calloc.free(nativeSecondValue);
    }
  }

  /// Opens the platform-specific core library without exposing FFI to widgets.
  static DynamicLibrary _openCoreLibrary() {
    if (Platform.isAndroid || Platform.isLinux) {
      return DynamicLibrary.open('libnote_taker_core.so');
    }
    if (Platform.isWindows) {
      return DynamicLibrary.open('note_taker_core.dll');
    }
    throw UnsupportedError(
        'The Rust Voice Gallery bridge is unavailable here.');
  }
}

typedef _NativeGetUnassignedClips = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DartGetUnassignedClips = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _NativeConfirmWeakMatch = Int32 Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _DartConfirmWeakMatch = int Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _NativeAssignNewIdentity = Int32 Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _DartAssignNewIdentity = int Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _DartTwoStringCommand = int Function(Pointer<Utf8>, Pointer<Utf8>);
typedef _NativeFreeString = Void Function(Pointer<Utf8>);
typedef _DartFreeString = void Function(Pointer<Utf8>);

const int _ffiSuccess = 0;
