// Coordinates Voice Gallery cluster-review state through an injectable FFI and playback boundary.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';

/// Supplies a composition-root configured controller to Riverpod consumers.
final voiceGalleryControllerProvider = ChangeNotifierProvider.autoDispose
    .family<VoiceGalleryController, String>((Ref ref, String meetingId) {
  return VoiceGalleryController(
    voiceGalleryBridge: RustVoiceGalleryBridge(),
    meetingId: meetingId,
  );
});

/// Owns displayable Voice Gallery state while keeping widgets free of native calls.
final class VoiceGalleryController extends ChangeNotifier {
  /// Creates a controller for the whole gallery, or for one meeting's voices when `meetingId` is
  /// given.
  VoiceGalleryController({
    required VoiceGalleryBridge voiceGalleryBridge,
    this.meetingId,
  }) : _voiceGalleryBridge = voiceGalleryBridge {
    _playbackFinished =
        voiceGalleryBridge.playbackFinished.listen((void _) => _markStopped());
  }

  final VoiceGalleryBridge _voiceGalleryBridge;

  /// Clears the playing mark when a clip reaches its end.
  late final StreamSubscription<void> _playbackFinished;

  String? _playingClusterId;

  /// The one meeting whose voices are shown, or null for every meeting's (Issue #63).
  final String? meetingId;

  List<UnknownSpeakerCluster> _clusters = const <UnknownSpeakerCluster>[];
  List<SavedPerson> _savedPeople = const <SavedPerson>[];
  bool _isLoading = false;
  String? _errorMessage;
  String? _statusMessage;

  /// Returns unresolved cluster metadata in creation order.
  List<UnknownSpeakerCluster> get clusters => _clusters;

  /// Returns the saved people, sorted by name.
  List<SavedPerson> get savedPeople => _savedPeople;

  /// Indicates whether an asynchronous gallery operation is active.
  bool get isLoading => _isLoading;

  /// Returns the cluster whose clip is playing, if any.
  String? get playingClusterId => _playingClusterId;

  /// Returns a display-safe loading failure, if any.
  String? get errorMessage => _errorMessage;

  /// Returns feedback about the most recent assignment or playback attempt, if any.
  String? get statusMessage => _statusMessage;

  /// Loads the gallery from the native boundary.
  ///
  /// Side effects: Replaces the lists and notifies listeners; a failure sets `errorMessage`.
  Future<void> load() async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final VoiceLibrary library = await _voiceGalleryBridge.getVoiceLibrary();
      _clusters = library.unknownClusters
          .where((UnknownSpeakerCluster cluster) =>
              meetingId == null || cluster.meetingId == meetingId)
          .toList(growable: false);
      _savedPeople = library.savedPeople;
    } on Object {
      _errorMessage = 'Unable to load Voice Gallery';
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  /// Plays a cluster's sample clip when it has one, or stops it when it is the one playing.
  ///
  /// Side effects: Stops any other clip, asks Rust to write this clip to a private file, then plays
  /// that file and marks this cluster as playing until it ends. A failure reports a status message.
  Future<void> play(UnknownSpeakerCluster cluster) async {
    if (!cluster.hasReviewClip) {
      return;
    }
    final bool wasPlaying = _playingClusterId == cluster.clusterId;
    try {
      if (_playingClusterId != null) {
        await _voiceGalleryBridge.stopPlayback();
        _markStopped();
      }
      if (wasPlaying) {
        return;
      }
      final String clipPath =
          await _voiceGalleryBridge.prepareReviewClip(cluster.clusterId);
      _playingClusterId = cluster.clusterId;
      notifyListeners();
      await _voiceGalleryBridge.playLocalFile(clipPath);
    } on Object {
      _playingClusterId = null;
      _statusMessage = 'Unable to play this voice';
      notifyListeners();
    }
  }

  /// Clears the playing mark.
  void _markStopped() {
    if (_playingClusterId != null) {
      _playingClusterId = null;
      notifyListeners();
    }
  }

  /// Deletes an unknown voice for good and refreshes the lists.
  ///
  /// Side effects: Stops its clip if it is playing, and deletes it in Rust. A refusal keeps the
  /// list and reports a status message.
  Future<void> deleteUnknownVoice(UnknownSpeakerCluster cluster) async {
    try {
      if (_playingClusterId == cluster.clusterId) {
        await _voiceGalleryBridge.stopPlayback();
        _markStopped();
      }
      await _voiceGalleryBridge.deleteUnknownVoice(cluster.clusterId);
    } on Object {
      _statusMessage = 'Unable to delete this voice';
      notifyListeners();
      return;
    }
    _statusMessage = 'Voice deleted';
    await load();
  }

  /// Deletes a saved person for good and refreshes the lists.
  ///
  /// Side effects: Deletes their name and voice samples in Rust. A refusal keeps the list and
  /// reports a status message.
  Future<void> deleteSavedPerson(SavedPerson person) async {
    try {
      await _voiceGalleryBridge.deleteSavedPerson(person.identityId);
    } on Object {
      _statusMessage = 'Unable to delete this person';
      notifyListeners();
      return;
    }
    _statusMessage = '${person.displayName} deleted';
    await load();
  }

  /// Assigns `cluster` to the identity named `identityName` and refreshes the list.
  ///
  /// A blank name is rejected locally without calling Rust. A rejected assignment keeps the
  /// current list and reports a status message.
  Future<void> assignCluster(
      UnknownSpeakerCluster cluster, String identityName) async {
    final String trimmedName = identityName.trim();
    if (trimmedName.isEmpty) {
      _statusMessage = 'Enter a name before assigning';
      notifyListeners();
      return;
    }
    try {
      final ClusterAssignmentResult result =
          await _voiceGalleryBridge.assignCluster(
        clusterId: cluster.clusterId,
        identityName: trimmedName,
      );
      _statusMessage = result.variantsStored > 0
          ? 'Assigned to ${result.identityName}'
          : 'Assigned to ${result.identityName}. There was not enough clear '
              'speech to recognize this voice in future recordings.';
    } on Object {
      _statusMessage = 'Unable to assign the speaker cluster';
      notifyListeners();
      return;
    }
    await load();
  }

  @override
  void dispose() {
    unawaited(_playbackFinished.cancel());
    super.dispose();
  }
}
