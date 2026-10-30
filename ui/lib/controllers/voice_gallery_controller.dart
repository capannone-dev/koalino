// Coordinates Voice Gallery cluster-review state through an injectable FFI and playback boundary.

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
  }) : _voiceGalleryBridge = voiceGalleryBridge;

  final VoiceGalleryBridge _voiceGalleryBridge;

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

  /// Plays a cluster's sample clip when it has one.
  ///
  /// Side effects: Asks Rust to write the clip to a private file, then plays that file. A failure
  /// reports a status message.
  Future<void> play(UnknownSpeakerCluster cluster) async {
    if (!cluster.hasReviewClip) {
      return;
    }
    try {
      await _voiceGalleryBridge.playLocalFile(
          await _voiceGalleryBridge.prepareReviewClip(cluster.clusterId));
    } on Object {
      _statusMessage = 'Unable to play this voice';
      notifyListeners();
    }
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
      _statusMessage = 'Assigned to ${result.identityName}';
    } on Object {
      _statusMessage = 'Unable to assign the speaker cluster';
      notifyListeners();
      return;
    }
    await load();
  }
}
