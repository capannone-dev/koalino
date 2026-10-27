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
  /// Creates a controller that loads clusters for one stable meeting identifier.
  VoiceGalleryController({
    required VoiceGalleryBridge voiceGalleryBridge,
    required this.meetingId,
  }) : _voiceGalleryBridge = voiceGalleryBridge;

  final VoiceGalleryBridge _voiceGalleryBridge;

  /// The meeting whose unresolved unknown-speaker clusters this controller presents.
  final String meetingId;

  List<UnknownSpeakerCluster> _clusters = const <UnknownSpeakerCluster>[];
  bool _isLoading = false;
  String? _errorMessage;
  String? _statusMessage;

  /// Returns unresolved cluster metadata in creation order.
  List<UnknownSpeakerCluster> get clusters => _clusters;

  /// Indicates whether an asynchronous gallery operation is active.
  bool get isLoading => _isLoading;

  /// Returns a display-safe loading failure, if any.
  String? get errorMessage => _errorMessage;

  /// Returns feedback about the most recent assignment attempt, if any.
  String? get statusMessage => _statusMessage;

  /// Loads the meeting's unresolved clusters from the native boundary.
  Future<void> load() async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      _clusters = await _voiceGalleryBridge.getUnknownClusters(meetingId);
    } on Object {
      _errorMessage = 'Unable to load Voice Gallery';
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  /// Plays a cluster's sample clip when Rust supplied one.
  Future<void> play(UnknownSpeakerCluster cluster) async {
    final String? reviewClipPath = cluster.reviewClipPath;
    if (reviewClipPath != null) {
      await _voiceGalleryBridge.playLocalFile(reviewClipPath);
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
