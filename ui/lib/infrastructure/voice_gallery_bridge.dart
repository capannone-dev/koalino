// Defines the Dart-native boundary for unknown-speaker clusters and local review playback.

import 'dart:convert';
import 'dart:ffi';
import 'dart:io';
import 'dart:isolate';

import 'package:audioplayers/audioplayers.dart';
import 'package:ffi/ffi.dart';

/// Describes one unresolved unknown-speaker cluster without biometric vectors.
final class UnknownSpeakerCluster {
  /// Creates immutable cluster metadata returned by the Rust boundary.
  const UnknownSpeakerCluster({
    required this.clusterId,
    required this.meetingId,
    required this.totalDurationMs,
    required this.chunkCount,
    this.reviewClipPath,
  });

  /// Stable cluster UUID sent back when assigning an identity.
  final String clusterId;

  /// Meeting that owns the cluster.
  final String meetingId;

  /// Summed speech duration of every chunk in the cluster.
  final int totalDurationMs;

  /// Number of voice chunks grouped into the cluster.
  final int chunkCount;

  /// Absolute native path of the cluster's sample clip, when one is still stored. Audio bytes
  /// never cross the FFI boundary.
  final String? reviewClipPath;

  /// Decodes one element of the `note_taker_get_unknown_clusters` JSON array.
  factory UnknownSpeakerCluster.fromJson(Map<String, Object?> json) {
    return UnknownSpeakerCluster(
      clusterId: json['cluster_id']! as String,
      meetingId: json['meeting_id']! as String,
      totalDurationMs: (json['total_duration_ms']! as num).toInt(),
      chunkCount: (json['chunk_count']! as num).toInt(),
      reviewClipPath: json['review_clip_path'] as String?,
    );
  }
}

/// Summarizes a completed cluster-to-identity assignment.
final class ClusterAssignmentResult {
  /// Creates an immutable assignment summary.
  const ClusterAssignmentResult({
    required this.clusterId,
    required this.identityId,
    required this.identityName,
    required this.identityCreated,
    required this.variantsStored,
    required this.segmentsUpdated,
  });

  /// The resolved cluster.
  final String clusterId;

  /// Identity the cluster was assigned to.
  final String identityId;

  /// Display name of that identity.
  final String identityName;

  /// Whether the identity was created by this assignment.
  final bool identityCreated;

  /// Number of voice variants stored for the identity.
  final int variantsStored;

  /// Number of transcript segments re-attributed to the identity.
  final int segmentsUpdated;
}

/// Decodes the `note_taker_get_unknown_clusters` response.
///
/// Returns the clusters for a JSON array; throws `StateError` carrying the Rust error for an
/// `{"ok":false,...}` object and `FormatException` for any other shape.
List<UnknownSpeakerCluster> decodeUnknownClusters(String responseJson) {
  final Object? decodedResponse = jsonDecode(responseJson);
  if (decodedResponse is List) {
    return decodedResponse
        .cast<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> json) =>
            UnknownSpeakerCluster.fromJson(json.cast<String, Object?>()))
        .toList(growable: false);
  }
  if (decodedResponse is Map) {
    throw _nativeError(decodedResponse.cast<String, Object?>());
  }
  throw const FormatException('Cluster response must be a JSON array.');
}

/// Decodes the `note_taker_assign_cluster` response.
///
/// Returns the summary for `{"ok":true,...}`; throws `StateError` carrying the Rust error code
/// and message otherwise.
ClusterAssignmentResult decodeClusterAssignment(String responseJson) {
  final Object? decodedResponse = jsonDecode(responseJson);
  if (decodedResponse is! Map) {
    throw const FormatException('Assignment response must be a JSON object.');
  }
  final Map<String, Object?> json = decodedResponse.cast<String, Object?>();
  if (json['ok'] != true) {
    throw _nativeError(json);
  }
  return ClusterAssignmentResult(
    clusterId: json['cluster_id']! as String,
    identityId: json['identity_id']! as String,
    identityName: json['identity_name']! as String,
    identityCreated: json['identity_created']! as bool,
    variantsStored: (json['variants_stored']! as num).toInt(),
    segmentsUpdated: (json['segments_updated']! as num).toInt(),
  );
}

/// Builds the exception for a Rust `{"ok":false,"error_code","error"}` response.
StateError _nativeError(Map<String, Object?> json) =>
    StateError('Rust rejected the Voice Gallery request '
        '(${json['error_code'] ?? 'unknown_error'}): ${json['error'] ?? ''}');

/// Declares Voice Gallery operations available to presentation and controller code.
abstract interface class VoiceGalleryBridge {
  /// Loads the unresolved unknown-speaker clusters belonging to `meetingId`.
  Future<List<UnknownSpeakerCluster>> getUnknownClusters(String meetingId);

  /// Assigns every chunk of `clusterId` to the identity named `identityName`.
  Future<ClusterAssignmentResult> assignCluster({
    required String clusterId,
    required String identityName,
  });

  /// Delegates file playback to the platform audio package.
  Future<void> playLocalFile(String localFilePath);
}

/// Implements Voice Gallery operations with C ABI calls and native file playback.
///
/// Native cluster calls read and write SQLite synchronously, so each one runs on a short-lived
/// background isolate that opens the platform core library itself.
final class RustVoiceGalleryBridge implements VoiceGalleryBridge {
  /// Creates a bridge that plays review clips through `audioPlayer`.
  RustVoiceGalleryBridge({AudioPlayer? audioPlayer})
      : _audioPlayer = audioPlayer ?? AudioPlayer();

  final AudioPlayer _audioPlayer;

  @override
  Future<List<UnknownSpeakerCluster>> getUnknownClusters(
      String meetingId) async {
    return decodeUnknownClusters(await _fetchUnknownClustersJson(meetingId));
  }

  @override
  Future<ClusterAssignmentResult> assignCluster({
    required String clusterId,
    required String identityName,
  }) async {
    return decodeClusterAssignment(
        await _assignClusterJson(clusterId, identityName));
  }

  @override
  Future<void> playLocalFile(String localFilePath) async {
    await _audioPlayer.play(DeviceFileSource(localFilePath));
  }
}

/// Calls `note_taker_get_unknown_clusters` on a background isolate.
///
/// Declared at top level so the isolate closure captures only the meeting ID string.
Future<String> _fetchUnknownClustersJson(String meetingId) =>
    Isolate.run(() => _callNativeJson(
          'note_taker_get_unknown_clusters',
          <String>[meetingId],
        ));

/// Calls `note_taker_assign_cluster` on a background isolate.
///
/// Declared at top level so the isolate closure captures only the two strings.
Future<String> _assignClusterJson(String clusterId, String identityName) =>
    Isolate.run(() => _callNativeJson(
          'note_taker_assign_cluster',
          <String>[clusterId, identityName],
        ));

/// Invokes a one- or two-string C command that returns caller-freed JSON and copies it out.
///
/// Throws `StateError` when Rust returns null (the response could not be allocated).
String _callNativeJson(String symbolName, List<String> arguments) {
  final DynamicLibrary coreLibrary = _openCoreLibrary();
  final _DartFreeString freeString =
      coreLibrary.lookupFunction<_NativeFreeString, _DartFreeString>(
          'note_taker_free_string');
  final List<Pointer<Utf8>> nativeArguments = arguments
      .map((String argument) => argument.toNativeUtf8())
      .toList(growable: false);
  try {
    final Pointer<Utf8> responsePointer = switch (nativeArguments) {
      [final Pointer<Utf8> onlyArgument] =>
        coreLibrary.lookupFunction<_NativeOneStringJson, _DartOneStringJson>(
            symbolName)(onlyArgument),
      [final Pointer<Utf8> firstArgument, final Pointer<Utf8> secondArgument] =>
        coreLibrary.lookupFunction<_NativeTwoStringJson, _DartTwoStringJson>(
            symbolName)(firstArgument, secondArgument),
      _ => throw ArgumentError.value(
          arguments.length, 'arguments', 'expected one or two arguments'),
    };
    if (responsePointer == nullptr) {
      throw StateError('Rust returned no response from $symbolName.');
    }
    try {
      return responsePointer.toDartString();
    } finally {
      freeString(responsePointer);
    }
  } finally {
    for (final Pointer<Utf8> nativeArgument in nativeArguments) {
      calloc.free(nativeArgument);
    }
  }
}

/// Opens the platform-specific core library without exposing FFI to widgets.
DynamicLibrary _openCoreLibrary() {
  if (Platform.isAndroid || Platform.isLinux) {
    return DynamicLibrary.open('libnote_taker_core.so');
  }
  if (Platform.isWindows) {
    return DynamicLibrary.open('note_taker_core.dll');
  }
  throw UnsupportedError('The Rust Voice Gallery bridge is unavailable here.');
}

typedef _NativeOneStringJson = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DartOneStringJson = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _NativeTwoStringJson = Pointer<Utf8> Function(
    Pointer<Utf8>, Pointer<Utf8>);
typedef _DartTwoStringJson = Pointer<Utf8> Function(
    Pointer<Utf8>, Pointer<Utf8>);
typedef _NativeFreeString = Void Function(Pointer<Utf8>);
typedef _DartFreeString = void Function(Pointer<Utf8>);
