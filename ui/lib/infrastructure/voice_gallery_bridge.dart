// Defines the Dart-native boundary for unknown-speaker clusters and local review playback.

import 'dart:convert';
import 'dart:ffi';
import 'dart:isolate';

import 'package:audioplayers/audioplayers.dart';
import 'package:ffi/ffi.dart';

import 'core_library.dart';

/// Describes one unresolved unknown-speaker cluster without biometric vectors.
final class UnknownSpeakerCluster {
  /// Creates immutable cluster metadata returned by the Rust boundary.
  const UnknownSpeakerCluster({
    required this.clusterId,
    required this.meetingId,
    required this.totalDurationMs,
    required this.chunkCount,
    this.meetingTitle,
    this.hasReviewClip = false,
    String? voiceGroupId,
    this.suggestedName,
    this.speakerLabel,
  }) : voiceGroupId = voiceGroupId ?? clusterId;

  /// Stable cluster UUID sent back when assigning an identity.
  final String clusterId;

  /// Meeting that owns the cluster.
  final String meetingId;

  /// The meeting's title, or null when it is untitled.
  final String? meetingTitle;

  /// Summed speech duration of every chunk in the cluster.
  final int totalDurationMs;

  /// Number of voice chunks grouped into the cluster.
  final int chunkCount;

  /// Whether a sample clip can still be played. Rust writes the clip to a file only on request,
  /// and audio bytes never cross the FFI boundary.
  final bool hasReviewClip;

  /// The group of voices that sound like this person (#82); the cluster's own ID when alone.
  final String voiceGroupId;

  /// The name the call window or the Teams captions showed while this voice spoke (#113), or
  /// null. It is only offered: nothing is named until the owner assigns it.
  final String? suggestedName;

  /// The label its meeting's transcript shows for this voice, such as `Speaker 2` (#118), or null
  /// from an older core.
  final String? speakerLabel;

  /// Decodes one element of the library's `unknown_clusters` array.
  factory UnknownSpeakerCluster.fromJson(Map<String, Object?> json) {
    return UnknownSpeakerCluster(
      clusterId: json['cluster_id']! as String,
      meetingId: json['meeting_id']! as String,
      meetingTitle: json['meeting_title'] as String?,
      totalDurationMs: (json['total_duration_ms']! as num).toInt(),
      chunkCount: (json['chunk_count']! as num).toInt(),
      hasReviewClip: json['has_review_clip'] as bool? ?? false,
      voiceGroupId: json['voice_group_id'] as String?,
      suggestedName: json['suggested_name'] as String?,
      speakerLabel: json['speaker_label'] as String?,
    );
  }
}

/// Voices that Rust found to sound like one person, shown and named together (#82).
final class UnknownVoiceGroup {
  /// Creates a group of at least one voice.
  const UnknownVoiceGroup({required this.groupId, required this.voices});

  /// The group's ID, shared by its voices.
  final String groupId;

  /// The group's voices, in library order.
  final List<UnknownSpeakerCluster> voices;

  /// Speech of every voice in the group, in milliseconds.
  int get totalDurationMs => voices.fold(
      0, (int sum, UnknownSpeakerCluster voice) => sum + voice.totalDurationMs);

  /// The first name the call window showed for one of the voices (#113), or null.
  String? get suggestedName {
    for (final UnknownSpeakerCluster voice in voices) {
      if (voice.suggestedName != null) {
        return voice.suggestedName;
      }
    }
    return null;
  }

  /// The different meetings the voices come from, in order.
  List<String> get meetingIds => <String>{
        for (final UnknownSpeakerCluster voice in voices) voice.meetingId
      }.toList(growable: false);

  /// Collects `voices` into groups by `voiceGroupId`, keeping the order of each group's first voice.
  static List<UnknownVoiceGroup> collect(List<UnknownSpeakerCluster> voices) {
    final Map<String, List<UnknownSpeakerCluster>> byGroup =
        <String, List<UnknownSpeakerCluster>>{};
    for (final UnknownSpeakerCluster voice in voices) {
      byGroup
          .putIfAbsent(voice.voiceGroupId, () => <UnknownSpeakerCluster>[])
          .add(voice);
    }
    return <UnknownVoiceGroup>[
      for (final MapEntry<String, List<UnknownSpeakerCluster>> entry
          in byGroup.entries)
        UnknownVoiceGroup(
            groupId: entry.key,
            voices: List<UnknownSpeakerCluster>.unmodifiable(entry.value)),
    ];
  }
}

/// Describes one saved person: how much voice data is held, but none of the data itself.
final class SavedPerson {
  /// Creates immutable person metadata returned by the Rust boundary.
  const SavedPerson({
    required this.identityId,
    required this.displayName,
    required this.voiceVariantCount,
  });

  /// Stable identity UUID.
  final String identityId;

  /// The person's name.
  final String displayName;

  /// Number of voice samples stored for this person.
  final int voiceVariantCount;
}

/// The Voice Gallery's contents: voices to name and people already saved.
final class VoiceLibrary {
  /// Creates an immutable library snapshot.
  const VoiceLibrary(
      {required this.unknownClusters, required this.savedPeople});

  /// Unresolved clusters across all meetings.
  final List<UnknownSpeakerCluster> unknownClusters;

  /// Saved people, sorted by name.
  final List<SavedPerson> savedPeople;
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

/// Summarizes several voices named as one person (#82).
final class VoiceGroupAssignmentResult {
  /// Creates an immutable summary.
  const VoiceGroupAssignmentResult({
    required this.identityId,
    required this.identityName,
    required this.identityCreated,
    required this.assignedClusterIds,
    required this.skippedClusterCount,
    required this.variantsStored,
    required this.segmentsUpdated,
  });

  /// Identity every voice was assigned to.
  final String identityId;

  /// Display name of that identity.
  final String identityName;

  /// Whether the identity was created by this naming.
  final bool identityCreated;

  /// The voices now named.
  final List<String> assignedClusterIds;

  /// Voices skipped because they were already named or gone.
  final int skippedClusterCount;

  /// Number of voice samples stored in all.
  final int variantsStored;

  /// Number of transcript segments re-attributed in all.
  final int segmentsUpdated;
}

/// Throws for a Rust error object, or returns `responseJson` decoded as a successful object.
Map<String, Object?> _decodeOkObject(String responseJson) {
  final Object? decodedResponse = jsonDecode(responseJson);
  if (decodedResponse is! Map) {
    throw const FormatException(
        'Voice Gallery response must be a JSON object.');
  }
  final Map<String, Object?> json = decodedResponse.cast<String, Object?>();
  if (json['ok'] != true) {
    throw _nativeError(json);
  }
  return json;
}

/// Decodes the `note_taker_get_voice_library` response.
///
/// Returns the library for `{"ok":true,...}`; throws `StateError` carrying the Rust error for
/// `{"ok":false,...}` and `FormatException` for any other shape.
VoiceLibrary decodeVoiceLibrary(String responseJson) {
  final Map<String, Object?> json = _decodeOkObject(responseJson);
  return VoiceLibrary(
    unknownClusters: (json['unknown_clusters']! as List<Object?>)
        .cast<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> cluster) =>
            UnknownSpeakerCluster.fromJson(cluster.cast<String, Object?>()))
        .toList(growable: false),
    savedPeople: (json['saved_people']! as List<Object?>)
        .cast<Map<Object?, Object?>>()
        .map((Map<Object?, Object?> person) => SavedPerson(
              identityId: person['identity_id']! as String,
              displayName: person['display_name']! as String,
              voiceVariantCount:
                  (person['voice_variant_count']! as num).toInt(),
            ))
        .toList(growable: false),
  );
}

/// Decodes the `note_taker_prepare_cluster_review_clip` response into the clip's file path.
///
/// Throws `StateError` carrying the Rust error for `{"ok":false,...}`.
String decodeReviewClipPath(String responseJson) =>
    _decodeOkObject(responseJson)['review_clip_path']! as String;

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

/// Decodes the `note_taker_assign_voice_group` response.
///
/// Returns the summary for `{"ok":true,...}`; throws `StateError` carrying the Rust error code
/// and message otherwise, and `FormatException` for any other shape.
VoiceGroupAssignmentResult decodeVoiceGroupAssignment(String responseJson) {
  final Map<String, Object?> json = _decodeOkObject(responseJson);
  return VoiceGroupAssignmentResult(
    identityId: json['identity_id']! as String,
    identityName: json['identity_name']! as String,
    identityCreated: json['identity_created']! as bool,
    assignedClusterIds: (json['assigned_cluster_ids']! as List<Object?>)
        .cast<String>()
        .toList(growable: false),
    skippedClusterCount: (json['skipped_cluster_count']! as num).toInt(),
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
  /// Loads every meeting's unresolved clusters and the saved people.
  Future<VoiceLibrary> getVoiceLibrary();

  /// Writes `clusterId`'s sample clip to a private file and returns its path.
  Future<String> prepareReviewClip(String clusterId);

  /// Assigns every chunk of `clusterId` to the identity named `identityName`.
  Future<ClusterAssignmentResult> assignCluster({
    required String clusterId,
    required String identityName,
  });

  /// Assigns every voice in `clusterIds` to one identity named `identityName` (#82).
  Future<VoiceGroupAssignmentResult> assignVoiceGroup({
    required List<String> clusterIds,
    required String identityName,
  });

  /// Delegates file playback to the platform audio package.
  Future<void> playLocalFile(String localFilePath);

  /// Stops the clip playing, if any.
  Future<void> stopPlayback();

  /// Emits each time a clip plays to its end.
  Stream<void> get playbackFinished;

  /// Deletes an unknown voice for good, here and on paired devices.
  ///
  /// Throws `StateError` when Rust refuses; a voice already gone counts as deleted.
  Future<void> deleteUnknownVoice(String clusterId);

  /// Deletes a saved person's name and voice samples for good, here and on paired devices.
  ///
  /// Throws `StateError` when Rust refuses.
  Future<void> deleteSavedPerson(String identityId);
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
  Future<VoiceLibrary> getVoiceLibrary() async =>
      decodeVoiceLibrary(await _fetchVoiceLibraryJson());

  @override
  Future<String> prepareReviewClip(String clusterId) async =>
      decodeReviewClipPath(await _prepareReviewClipJson(clusterId));

  @override
  Future<ClusterAssignmentResult> assignCluster({
    required String clusterId,
    required String identityName,
  }) async {
    return decodeClusterAssignment(
        await _assignClusterJson(clusterId, identityName));
  }

  @override
  Future<VoiceGroupAssignmentResult> assignVoiceGroup({
    required List<String> clusterIds,
    required String identityName,
  }) async {
    return decodeVoiceGroupAssignment(
        await _assignVoiceGroupJson(jsonEncode(clusterIds), identityName));
  }

  @override
  Future<void> playLocalFile(String localFilePath) async {
    await _audioPlayer.play(DeviceFileSource(localFilePath));
  }

  @override
  Future<void> stopPlayback() => _audioPlayer.stop();

  @override
  Stream<void> get playbackFinished => _audioPlayer.onPlayerComplete;

  @override
  Future<void> deleteUnknownVoice(String clusterId) =>
      _deleteByStatus('note_taker_delete_unknown_voice', clusterId);

  @override
  Future<void> deleteSavedPerson(String identityId) =>
      _deleteByStatus('note_taker_delete_saved_person', identityId);
}

/// Calls a native delete function on a background isolate, because it writes to SQLite.
///
/// Inputs: The symbol and the ID it takes. Outputs: Completes when the item is gone, also when it
/// was already (status 5). Throws `StateError` for any other refusal.
Future<void> _deleteByStatus(String symbolName, String itemId) async {
  final int statusCode = await Isolate.run(() {
    final Pointer<Utf8> nativeItemId = itemId.toNativeUtf8();
    try {
      return _openCoreLibrary()
          .lookupFunction<_NativeDelete, _DartDelete>(symbolName)(nativeItemId);
    } finally {
      calloc.free(nativeItemId);
    }
  });
  if (statusCode != 0 && statusCode != 5) {
    throw StateError('Rust refused the deletion (status $statusCode).');
  }
}

/// Calls `note_taker_get_voice_library` on a background isolate.
Future<String> _fetchVoiceLibraryJson() => Isolate.run(
    () => _callNativeJson('note_taker_get_voice_library', const <String>[]));

/// Calls `note_taker_prepare_cluster_review_clip` on a background isolate.
///
/// Declared at top level so the isolate closure captures only the cluster ID string.
Future<String> _prepareReviewClipJson(String clusterId) =>
    Isolate.run(() => _callNativeJson(
          'note_taker_prepare_cluster_review_clip',
          <String>[clusterId],
        ));

/// Calls `note_taker_assign_cluster` on a background isolate.
///
/// Declared at top level so the isolate closure captures only the two strings.
Future<String> _assignClusterJson(String clusterId, String identityName) =>
    Isolate.run(() => _callNativeJson(
          'note_taker_assign_cluster',
          <String>[clusterId, identityName],
        ));

/// Calls `note_taker_assign_voice_group` on a background isolate.
///
/// Declared at top level so the isolate closure captures only the two strings.
Future<String> _assignVoiceGroupJson(
        String clusterIdsJson, String identityName) =>
    Isolate.run(() => _callNativeJson(
          'note_taker_assign_voice_group',
          <String>[clusterIdsJson, identityName],
        ));

/// Invokes a zero-, one- or two-string C command that returns caller-freed JSON and copies it out.
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
      [] =>
        coreLibrary.lookupFunction<_NativeNoArgumentJson, _DartNoArgumentJson>(
            symbolName)(),
      [final Pointer<Utf8> onlyArgument] =>
        coreLibrary.lookupFunction<_NativeOneStringJson, _DartOneStringJson>(
            symbolName)(onlyArgument),
      [final Pointer<Utf8> firstArgument, final Pointer<Utf8> secondArgument] =>
        coreLibrary.lookupFunction<_NativeTwoStringJson, _DartTwoStringJson>(
            symbolName)(firstArgument, secondArgument),
      _ => throw ArgumentError.value(
          arguments.length, 'arguments', 'expected at most two arguments'),
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
  return openCoreLibrary();
}

typedef _NativeDelete = Int32 Function(Pointer<Utf8>);
typedef _DartDelete = int Function(Pointer<Utf8>);
typedef _NativeNoArgumentJson = Pointer<Utf8> Function();
typedef _DartNoArgumentJson = Pointer<Utf8> Function();
typedef _NativeOneStringJson = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _DartOneStringJson = Pointer<Utf8> Function(Pointer<Utf8>);
typedef _NativeTwoStringJson = Pointer<Utf8> Function(
    Pointer<Utf8>, Pointer<Utf8>);
typedef _DartTwoStringJson = Pointer<Utf8> Function(
    Pointer<Utf8>, Pointer<Utf8>);
typedef _NativeFreeString = Void Function(Pointer<Utf8>);
typedef _DartFreeString = void Function(Pointer<Utf8>);
