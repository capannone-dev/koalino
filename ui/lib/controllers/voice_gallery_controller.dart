// Coordinates Voice Gallery cluster-review state through an injectable FFI and playback boundary.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

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
  List<UnknownVoiceGroup> _groups = const <UnknownVoiceGroup>[];

  /// Voices the owner left out of their group's naming (#82), by cluster ID.
  final Set<String> _leftOutClusterIds = <String>{};
  List<SavedPerson> _savedPeople = const <SavedPerson>[];
  bool _isLoading = false;
  String? _errorMessage;
  String? _statusMessage;

  /// Returns unresolved cluster metadata in creation order.
  List<UnknownSpeakerCluster> get clusters => _clusters;

  /// Returns the unresolved voices grouped by person, in the order Rust listed them (#82).
  List<UnknownVoiceGroup> get groups => _groups;

  /// Reports whether `cluster` is named with the rest of its group.
  bool isIncluded(UnknownSpeakerCluster cluster) =>
      !_leftOutClusterIds.contains(cluster.clusterId);

  /// Includes `cluster` in, or leaves it out of, its group's naming.
  ///
  /// Side effects: Notifies listeners. A voice left out stays listed and can be named later.
  void setIncluded(UnknownSpeakerCluster cluster, bool included) {
    if (included) {
      _leftOutClusterIds.remove(cluster.clusterId);
    } else {
      _leftOutClusterIds.add(cluster.clusterId);
    }
    notifyListeners();
  }

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
      _groups = UnknownVoiceGroup.collect(_clusters);
      _leftOutClusterIds.removeWhere((String clusterId) => !_clusters.any(
          (UnknownSpeakerCluster cluster) => cluster.clusterId == clusterId));
      _savedPeople = library.savedPeople;
    } on Object {
      _errorMessage = l10n.unableToLoadVoiceGallery;
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
      _statusMessage = l10n.unableToPlayThisVoice;
      notifyListeners();
    }
  }

  /// Plays the longest voice of `group` that has a sample, or stops it when it is playing.
  ///
  /// Side effects: As `play`.
  Future<void> playGroup(UnknownVoiceGroup group) async {
    final List<UnknownSpeakerCluster> playable = group.voices
        .where((UnknownSpeakerCluster voice) => voice.hasReviewClip)
        .toList();
    if (playable.isEmpty) {
      return;
    }
    final UnknownSpeakerCluster playing = playable.firstWhere(
        (UnknownSpeakerCluster voice) => voice.clusterId == _playingClusterId,
        orElse: () => playable.reduce((UnknownSpeakerCluster longest,
                UnknownSpeakerCluster voice) =>
            voice.totalDurationMs > longest.totalDurationMs ? voice : longest));
    await play(playing);
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
      _statusMessage = l10n.unableToDeleteThisVoice;
      notifyListeners();
      return;
    }
    _statusMessage = l10n.voiceDeleted;
    await load();
  }

  /// Deletes every voice of `group` for good and refreshes the lists.
  ///
  /// Side effects: Stops playback if one of them is playing, and deletes each in Rust. A refusal
  /// stops there, keeps what is left, and reports a status message.
  Future<void> deleteGroup(UnknownVoiceGroup group) async {
    try {
      if (group.voices.any((UnknownSpeakerCluster voice) =>
          voice.clusterId == _playingClusterId)) {
        await _voiceGalleryBridge.stopPlayback();
        _markStopped();
      }
      for (final UnknownSpeakerCluster voice in group.voices) {
        await _voiceGalleryBridge.deleteUnknownVoice(voice.clusterId);
      }
    } on Object {
      _statusMessage = l10n.unableToDeleteThisVoice;
      await load();
      return;
    }
    _statusMessage = l10n.voiceDeleted;
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
      _statusMessage = l10n.unableToDeleteThisPerson;
      notifyListeners();
      return;
    }
    _statusMessage = l10n.deleted(person.displayName);
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
      _statusMessage = l10n.enterANameBeforeAssigning;
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
          ? l10n.assignedTo(result.identityName)
          : l10n.assignedToThereWasNotEnough(result.identityName);
    } on Object {
      _statusMessage = l10n.unableToAssignTheSpeakerCluster;
      notifyListeners();
      return;
    }
    await load();
  }

  /// Names the included voices of `group` as one person and refreshes the list (#82).
  ///
  /// A one-voice group goes through `assignCluster`. A blank name, or no voice included, is
  /// rejected locally without calling Rust. A rejected naming keeps the list and reports it.
  Future<void> assignGroup(UnknownVoiceGroup group, String identityName) async {
    if (group.voices.length == 1) {
      return assignCluster(group.voices.single, identityName);
    }
    final String trimmedName = identityName.trim();
    if (trimmedName.isEmpty) {
      _statusMessage = l10n.enterANameBeforeAssigning;
      notifyListeners();
      return;
    }
    final List<String> clusterIds = <String>[
      for (final UnknownSpeakerCluster voice in group.voices)
        if (isIncluded(voice)) voice.clusterId,
    ];
    if (clusterIds.isEmpty) {
      _statusMessage = l10n.chooseAtLeastOneVoiceTo;
      notifyListeners();
      return;
    }
    try {
      final VoiceGroupAssignmentResult result =
          await _voiceGalleryBridge.assignVoiceGroup(
        clusterIds: clusterIds,
        identityName: trimmedName,
      );
      final int named = result.assignedClusterIds.length;
      final String assigned = l10n.assignedVoicesTo(named, result.identityName);
      _statusMessage = result.variantsStored > 0
          ? assigned
          : l10n.thereWasNotEnoughClearSpeech(assigned);
    } on Object {
      _statusMessage = l10n.unableToAssignTheSpeakerCluster;
      notifyListeners();
      return;
    }
    await load();
  }

  /// Gives `voices` (unknown voices the owner picked) to the saved `person` (#136): their
  /// voice samples join that person's, so later meetings recognize them in these voices too.
  ///
  /// One voice goes through `assignCluster`, several through one group naming. An empty pick is
  /// rejected locally. Side effects: Calls Rust, then reloads the gallery.
  Future<void> addVoicesToPerson(
      List<UnknownSpeakerCluster> voices, SavedPerson person) async {
    if (voices.isEmpty) {
      _statusMessage = l10n.chooseAtLeastOneVoiceTo;
      notifyListeners();
      return;
    }
    if (voices.length == 1) {
      return assignCluster(voices.single, person.displayName);
    }
    try {
      final VoiceGroupAssignmentResult result =
          await _voiceGalleryBridge.assignVoiceGroup(
        clusterIds: <String>[
          for (final UnknownSpeakerCluster voice in voices) voice.clusterId,
        ],
        identityName: person.displayName,
      );
      final String assigned = l10n.assignedVoicesTo(
          result.assignedClusterIds.length, result.identityName);
      _statusMessage = result.variantsStored > 0
          ? assigned
          : l10n.thereWasNotEnoughClearSpeech(assigned);
    } on Object {
      _statusMessage = l10n.unableToAssignTheSpeakerCluster;
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
