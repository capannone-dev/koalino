// Coordinates Voice Gallery state through an injectable FFI and playback boundary.

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
  /// Creates a controller that loads clips for one stable meeting identifier.
  VoiceGalleryController({
    required VoiceGalleryBridge voiceGalleryBridge,
    required this.meetingId,
  }) : _voiceGalleryBridge = voiceGalleryBridge;

  final VoiceGalleryBridge _voiceGalleryBridge;

  /// The meeting whose unresolved clips this controller presents.
  final String meetingId;

  List<VoiceClip> _clips = const <VoiceClip>[];
  List<VoiceIdentity> _identities = const <VoiceIdentity>[];
  bool _isLoading = false;
  String? _errorMessage;

  /// Returns immutable review-clip metadata suitable for rendering.
  List<VoiceClip> get clips => _clips;

  /// Returns known identities loaded for manual assignment controls.
  List<VoiceIdentity> get identities => _identities;

  /// Indicates whether an asynchronous gallery operation is active.
  bool get isLoading => _isLoading;

  /// Returns a display-safe operation failure, if any.
  String? get errorMessage => _errorMessage;

  /// Loads unresolved clip metadata and selectable identities from the native boundary.
  Future<void> load() async {
    _isLoading = true;
    _errorMessage = null;
    notifyListeners();
    try {
      final List<Object> loadedValues =
          await Future.wait<Object>(<Future<Object>>[
        _voiceGalleryBridge.getUnassignedClips(meetingId),
        _voiceGalleryBridge.getVoiceIdentities(),
      ]);
      _clips = loadedValues[0] as List<VoiceClip>;
      _identities = loadedValues[1] as List<VoiceIdentity>;
    } on Object {
      _errorMessage = 'Unable to load Voice Gallery';
    } finally {
      _isLoading = false;
      notifyListeners();
    }
  }

  /// Plays one clip through its native local file path.
  Future<void> play(VoiceClip clip) =>
      _voiceGalleryBridge.playLocalFile(clip.localFilePath);

  /// Confirms the selected weak-match identity and refreshes the review list.
  Future<void> confirmWeakMatch(VoiceClip clip, String identityId) async {
    await _voiceGalleryBridge.confirmWeakMatch(
      clipId: clip.clipId,
      identityId: identityId,
    );
    await load();
  }

  /// Assigns a reviewed clip to an existing identity and refreshes unresolved clips.
  Future<void> assignExistingIdentity(VoiceClip clip, String identityId) async {
    await _voiceGalleryBridge.confirmWeakMatch(
      clipId: clip.clipId,
      identityId: identityId,
    );
    await load();
  }

  /// Creates an identity from `clip` and refreshes the unresolved review list.
  Future<void> createNewIdentity(VoiceClip clip, String displayName) async {
    await _voiceGalleryBridge.assignNewIdentity(
      clipId: clip.clipId,
      displayName: displayName,
    );
    await load();
  }
}
