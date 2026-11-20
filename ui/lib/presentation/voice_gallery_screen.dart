// Renders unknown voices with sample playback and name assignment, and the saved people. Opens
// from the Voices destination for all meetings, or from a meeting for that meeting's (Issue #63).
// Voices that sound like one person share a card and are named at once (#82).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/presentation/confirm_delete_dialog.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Groups with at most this many parts show them, each with its own Play, without a tap (#134).
const int maxPartsShownOpen = 6;

/// Displays the voice gallery using controller-owned state.
final class VoiceGalleryScreen extends StatefulWidget {
  /// Creates a Voice Gallery bound to an injected controller.
  const VoiceGalleryScreen({required this.voiceGalleryController, super.key});

  /// Owns asynchronous loading and all FFI-bound user actions.
  final VoiceGalleryController voiceGalleryController;

  @override
  State<VoiceGalleryScreen> createState() => _VoiceGalleryScreenState();
}

/// Starts loading once, keeps one name field per cluster, and renders controller changes.
final class _VoiceGalleryScreenState extends State<VoiceGalleryScreen> {
  /// Name-field controllers keyed by group ID (a lone voice's cluster ID), so typed text survives
  /// rebuilds.
  final Map<String, TextEditingController> _nameControllers =
      <String, TextEditingController>{};

  @override
  void initState() {
    super.initState();
    widget.voiceGalleryController.load();
  }

  @override
  void dispose() {
    for (final TextEditingController nameController
        in _nameControllers.values) {
      nameController.dispose();
    }
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(l10n.voiceGallery)),
      body: AnimatedBuilder(
        animation: widget.voiceGalleryController,
        builder: (BuildContext context, Widget? child) {
          final VoiceGalleryController controller =
              widget.voiceGalleryController;
          if (controller.isLoading &&
              controller.clusters.isEmpty &&
              controller.savedPeople.isEmpty) {
            return const Center(child: CircularProgressIndicator());
          }
          if (controller.errorMessage case final String errorMessage) {
            return Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  Text(errorMessage),
                  const SizedBox(height: 12),
                  OutlinedButton(
                    onPressed: controller.load,
                    child: Text(l10n.tryAgain),
                  ),
                ],
              ),
            );
          }
          _pruneNameControllers(controller.groups);
          final String? statusMessage = controller.statusMessage;
          final bool showsSavedPeople = controller.meetingId == null;
          return ListView(
            padding: const EdgeInsets.all(16),
            children: <Widget>[
              if (statusMessage != null)
                Padding(
                  padding: const EdgeInsets.only(bottom: 12),
                  child: Text(
                    statusMessage,
                    style: Theme.of(context).textTheme.bodyLarge,
                  ),
                ),
              if (showsSavedPeople) _sectionTitle(context, l10n.unknownVoices),
              if (controller.clusters.isEmpty)
                Padding(
                  padding: EdgeInsets.symmetric(vertical: 8),
                  child: Text(l10n.noUnknownSpeakersToReview),
                ),
              for (final (int index, UnknownVoiceGroup group)
                  in controller.groups.indexed)
                if (group.voices.length == 1)
                  _buildClusterCard(
                      context, controller, index, group.voices.single)
                else
                  _buildGroupCard(context, controller, index, group),
              if (showsSavedPeople) ...<Widget>[
                _sectionTitle(context, l10n.savedPeople),
                if (controller.savedPeople.isEmpty)
                  Padding(
                    padding: EdgeInsets.symmetric(vertical: 8),
                    child: Text(l10n.noSavedPeopleYetNameA),
                  ),
                for (final SavedPerson person in controller.savedPeople)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.person_outline),
                    title: Text(person.displayName),
                    subtitle:
                        Text(l10n.voiceSampleCount(person.voiceVariantCount)),
                    trailing: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: <Widget>[
                        if (controller.clusters.isNotEmpty)
                          TextButton.icon(
                            key: ValueKey<String>(
                                'add-voice-${person.identityId}'),
                            icon: const Icon(Icons.record_voice_over_outlined),
                            label: Text(l10n.addAVoice),
                            onPressed: () => _pickVoicesForPerson(
                                context, controller, person),
                          ),
                        IconButton(
                          key: ValueKey<String>(
                              'delete-person-${person.identityId}'),
                          tooltip: l10n.delete4(person.displayName),
                          icon: const Icon(Icons.delete_outline),
                          onPressed: () =>
                              _confirmDeletePerson(context, controller, person),
                        ),
                      ],
                    ),
                  ),
              ],
            ],
          );
        },
      ),
    );
  }

  /// Builds a section heading.
  Widget _sectionTitle(BuildContext context, String title) => Padding(
        padding: const EdgeInsets.only(top: 8, bottom: 8),
        child: Text(title,
            style: Theme.of(context)
                .textTheme
                .titleSmall
                ?.copyWith(color: Theme.of(context).colorScheme.primary)),
      );

  /// Builds one cluster card with its summary, optional Play action, and name assignment row.
  Widget _buildClusterCard(
    BuildContext context,
    VoiceGalleryController controller,
    int index,
    UnknownSpeakerCluster cluster,
  ) {
    final TextEditingController nameController = _nameControllers.putIfAbsent(
        cluster.clusterId,
        () => TextEditingController(text: cluster.suggestedName));
    final bool isPlaying = controller.playingClusterId == cluster.clusterId;
    final ColorScheme colors = Theme.of(context).colorScheme;
    return Card(
      key: ValueKey<String>('card-${cluster.clusterId}'),
      margin: const EdgeInsets.only(bottom: 12),
      color: isPlaying ? colors.primaryContainer : null,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: isPlaying
                  ? Icon(Icons.volume_up, color: colors.onPrimaryContainer)
                  : null,
              title: Text(_voiceTitle(cluster, index)),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  if (isPlaying)
                    Text(l10n.playing,
                        style: TextStyle(
                            color: colors.onPrimaryContainer,
                            fontWeight: FontWeight.bold)),
                  Text(_formatSummary(cluster)),
                  if (controller.meetingId == null)
                    Text(cluster.meetingTitle ?? l10n.untitledMeeting),
                  if (cluster.suggestedName != null)
                    _suggestionLine(cluster.clusterId, cluster.suggestedName!),
                ],
              ),
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (cluster.hasReviewClip)
                    TextButton(
                      key: ValueKey<String>('play-${cluster.clusterId}'),
                      onPressed: () => controller.play(cluster),
                      child: Text(isPlaying ? l10n.stop : l10n.play),
                    ),
                  IconButton(
                    key: ValueKey<String>('delete-${cluster.clusterId}'),
                    tooltip: l10n.deleteThisVoice,
                    icon: const Icon(Icons.delete_outline),
                    onPressed: () => _confirmDeleteVoice(
                        context, controller, index, cluster),
                  ),
                ],
              ),
            ),
            _nameRow(
              controller,
              nameController,
              cluster.clusterId,
              (String typedName) =>
                  controller.assignCluster(cluster, typedName),
            ),
          ],
        ),
      ),
    );
  }

  /// Says where a suggested name came from (#113); the name field is filled with it.
  Widget _suggestionLine(String keyId, String suggestedName) => Row(
        key: ValueKey<String>('suggested-$keyId'),
        children: <Widget>[
          const Icon(Icons.badge_outlined, size: 16),
          const SizedBox(width: 4),
          Flexible(
            child: Text(l10n.theCallShowedSpeakingCheckThen(suggestedName)),
          ),
        ],
      );

  /// Builds the name field, with saved people to pick from, and the Assign button.
  ///
  /// `keyId` names the widgets' keys; `onAssign` receives the typed name.
  Widget _nameRow(
    VoiceGalleryController controller,
    TextEditingController nameController,
    String keyId,
    void Function(String) onAssign,
  ) {
    final Widget newPersonRow = Row(
      children: <Widget>[
        Expanded(
          child: TextField(
            key: ValueKey<String>('identity-name-$keyId'),
            controller: nameController,
            decoration: InputDecoration(
              labelText: controller.savedPeople.isEmpty
                  ? l10n.name
                  : l10n.newPersonsName,
              border: const OutlineInputBorder(),
            ),
            textInputAction: TextInputAction.done,
            onSubmitted: onAssign,
          ),
        ),
        const SizedBox(width: 12),
        FilledButton(
          key: ValueKey<String>('assign-$keyId'),
          onPressed:
              controller.isLoading ? null : () => onAssign(nameController.text),
          child: Text(l10n.assign),
        ),
      ],
    );
    if (controller.savedPeople.isEmpty) {
      return newPersonRow;
    }
    // A voice that wasn't matched to someone already saved is given to them here (#136).
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: <Widget>[
        newPersonRow,
        const SizedBox(height: 8),
        Align(
          alignment: AlignmentDirectional.centerStart,
          child: MenuAnchor(
            menuChildren: <Widget>[
              for (final SavedPerson person in controller.savedPeople)
                MenuItemButton(
                  key: ValueKey<String>('add-to-$keyId-${person.identityId}'),
                  leadingIcon: const Icon(Icons.person_outline),
                  onPressed: controller.isLoading
                      ? null
                      : () => onAssign(person.displayName),
                  child: Text(person.displayName),
                ),
            ],
            builder:
                (BuildContext context, MenuController menu, Widget? child) =>
                    OutlinedButton.icon(
              key: ValueKey<String>('add-to-person-$keyId'),
              icon: const Icon(Icons.person_add_alt_1_outlined),
              label: Text(l10n.addToASavedPerson),
              onPressed: () => menu.isOpen ? menu.close() : menu.open(),
            ),
          ),
        ),
      ],
    );
  }

  /// Lets the owner tick unknown voices that are `person` and gives them to them (#136).
  ///
  /// Side effects: Shows a dialog; may play clips and assign voices through the controller.
  Future<void> _pickVoicesForPerson(BuildContext context,
      VoiceGalleryController controller, SavedPerson person) async {
    final List<UnknownSpeakerCluster>? picked =
        await showDialog<List<UnknownSpeakerCluster>>(
      context: context,
      builder: (BuildContext dialogContext) => _VoicePickerDialog(
        controller: controller,
        person: person,
        titleOf: (UnknownSpeakerCluster voice) =>
            _voiceTitle(voice, controller.clusters.indexOf(voice)),
        detailsOf: (UnknownSpeakerCluster voice) =>
            '${voice.meetingTitle ?? l10n.untitledMeeting} · '
            '${_formatSeconds(voice.totalDurationMs)}',
      ),
    );
    // A clip still playing from the dialog stops with it.
    for (final UnknownSpeakerCluster voice in controller.clusters) {
      if (voice.clusterId == controller.playingClusterId) {
        await controller.play(voice);
      }
    }
    if (picked != null && picked.isNotEmpty) {
      await controller.addVoicesToPerson(picked, person);
    }
  }

  /// Builds one card for several voices of one person (#82): their total, the meetings they
  /// come from, Play for the longest, the parts to leave out or play, and one name for all.
  Widget _buildGroupCard(
    BuildContext context,
    VoiceGalleryController controller,
    int index,
    UnknownVoiceGroup group,
  ) {
    final TextEditingController nameController = _nameControllers.putIfAbsent(
        group.groupId, () => TextEditingController(text: group.suggestedName));
    final bool isPlaying = group.voices.any((UnknownSpeakerCluster voice) =>
        voice.clusterId == controller.playingClusterId);
    final bool canPlay =
        group.voices.any((UnknownSpeakerCluster voice) => voice.hasReviewClip);
    final ColorScheme colors = Theme.of(context).colorScheme;
    final int meetingCount = group.meetingIds.length;
    final String partsLabel = l10n.partCount(group.voices.length);
    final String coverage = meetingCount == 1
        ? partsLabel
        : l10n.inMeetings(partsLabel, l10n.meetingCount(meetingCount));
    final List<String> meetingTitles = <String>{
      for (final UnknownSpeakerCluster voice in group.voices)
        voice.meetingTitle ?? l10n.untitledMeeting,
    }.toList(growable: false);
    return Card(
      key: ValueKey<String>('card-${group.groupId}'),
      margin: const EdgeInsets.only(bottom: 12),
      color: isPlaying ? colors.primaryContainer : null,
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            ListTile(
              contentPadding: EdgeInsets.zero,
              leading: isPlaying
                  ? Icon(Icons.volume_up, color: colors.onPrimaryContainer)
                  : null,
              title: Text(_groupTitle(group, index)),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  if (isPlaying)
                    Text(l10n.playing,
                        style: TextStyle(
                            color: colors.onPrimaryContainer,
                            fontWeight: FontWeight.bold)),
                  Text('${_formatSeconds(group.totalDurationMs)} · $coverage'),
                  if (controller.meetingId == null)
                    Text(meetingTitles.join(', ')),
                  if (group.suggestedName != null)
                    _suggestionLine(group.groupId, group.suggestedName!),
                ],
              ),
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (canPlay)
                    TextButton(
                      key: ValueKey<String>('play-group-${group.groupId}'),
                      onPressed: () => controller.playGroup(group),
                      child: Text(isPlaying ? l10n.stop : l10n.play),
                    ),
                  IconButton(
                    key: ValueKey<String>('delete-${group.groupId}'),
                    tooltip: l10n.deleteTheseVoices,
                    icon: const Icon(Icons.delete_outline),
                    onPressed: () =>
                        _confirmDeleteGroup(context, controller, index, group),
                  ),
                ],
              ),
            ),
            ExpansionTile(
              key: ValueKey<String>('parts-${group.groupId}'),
              // Open for a small group, so every part shows its own Play (#134).
              initiallyExpanded: group.voices.length <= maxPartsShownOpen,
              tilePadding: EdgeInsets.zero,
              title: Text(partsLabel),
              subtitle: Text(l10n.theseSoundLikeOnePersonUntick),
              children: <Widget>[
                for (final UnknownSpeakerCluster voice in group.voices)
                  _buildPartRow(controller, voice),
              ],
            ),
            const SizedBox(height: 8),
            _nameRow(
              controller,
              nameController,
              group.groupId,
              (String typedName) => controller.assignGroup(group, typedName),
            ),
          ],
        ),
      ),
    );
  }

  /// Builds one voice of a group: whether it is named with the others, where it is from, and Play.
  Widget _buildPartRow(
      VoiceGalleryController controller, UnknownSpeakerCluster voice) {
    final bool isPlaying = controller.playingClusterId == voice.clusterId;
    return Row(
      children: <Widget>[
        Checkbox(
          key: ValueKey<String>('include-${voice.clusterId}'),
          value: controller.isIncluded(voice),
          onChanged: (bool? included) =>
              controller.setIncluded(voice, included ?? false),
        ),
        Expanded(
          child: Text(
            '${voice.speakerLabel != null ? '${voice.speakerLabel} in ' : ''}'
            '${voice.meetingTitle ?? 'Untitled meeting'} · '
            '${_formatSeconds(voice.totalDurationMs)}',
          ),
        ),
        if (voice.hasReviewClip)
          TextButton(
            key: ValueKey<String>('play-${voice.clusterId}'),
            onPressed: () => controller.play(voice),
            child: Text(isPlaying ? l10n.stop : l10n.play),
          ),
      ],
    );
  }

  /// Asks before deleting every voice of a group, then deletes them.
  Future<void> _confirmDeleteGroup(
    BuildContext context,
    VoiceGalleryController controller,
    int index,
    UnknownVoiceGroup group,
  ) async {
    if (await confirmDelete(
      context,
      title: l10n.delete2(_groupTitle(group, index)),
      message: l10n.itsVoiceSamplesGoForGood(group.voices.length),
    )) {
      await controller.deleteGroup(group);
    }
  }

  /// Asks before deleting an unknown voice, then deletes it.
  Future<void> _confirmDeleteVoice(
    BuildContext context,
    VoiceGalleryController controller,
    int index,
    UnknownSpeakerCluster cluster,
  ) async {
    if (await confirmDelete(
      context,
      title: l10n.delete2(_voiceTitle(cluster, index)),
      message: l10n.itsVoiceSampleGoesForGood,
    )) {
      await controller.deleteUnknownVoice(cluster);
    }
  }

  /// Asks before deleting a saved person, then deletes them.
  Future<void> _confirmDeletePerson(
    BuildContext context,
    VoiceGalleryController controller,
    SavedPerson person,
  ) async {
    if (await confirmDelete(
      context,
      title: l10n.delete2(person.displayName),
      message: l10n.theirNameAndVoiceSamplesGo,
    )) {
      await controller.deleteSavedPerson(person);
    }
  }

  /// Forgets name fields whose groups are no longer listed and disposes them after the frame,
  /// once their `TextField`s have unmounted.
  void _pruneNameControllers(List<UnknownVoiceGroup> groups) {
    final Set<String> listedClusterIds =
        groups.map((UnknownVoiceGroup group) => group.groupId).toSet();
    final List<TextEditingController> staleControllers =
        <TextEditingController>[];
    _nameControllers
        .removeWhere((String clusterId, TextEditingController nameController) {
      if (listedClusterIds.contains(clusterId)) {
        return false;
      }
      staleControllers.add(nameController);
      return true;
    });
    if (staleControllers.isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((Duration _) {
        for (final TextEditingController staleController in staleControllers) {
          staleController.dispose();
        }
      });
    }
  }

  /// Names an unknown voice as its meeting's transcript does ("Speaker 2", #118); an older core
  /// without labels falls back to its place in the list.
  String _voiceTitle(UnknownSpeakerCluster cluster, int index) =>
      cluster.speakerLabel ?? l10n.unknownSpeaker(index + 1);

  /// Names a group of voices by their transcripts' labels, each once ("Speaker 1 / Speaker 3"
  /// when the voice was numbered differently in different meetings).
  String _groupTitle(UnknownVoiceGroup group, int index) {
    final List<String> labels = <String>{
      for (final UnknownSpeakerCluster voice in group.voices)
        if (voice.speakerLabel != null) voice.speakerLabel!,
    }.toList(growable: false);
    return labels.isEmpty ? l10n.unknownSpeaker(index + 1) : labels.join(' / ');
  }

  /// Formats total speech time (seconds, one decimal) and chunk count, e.g. `12.5 s · 3 chunks`.
  String _formatSummary(UnknownSpeakerCluster cluster) {
    return '${_formatSeconds(cluster.totalDurationMs)} · '
        '${l10n.chunkCount(cluster.chunkCount)}';
  }

  /// Formats milliseconds as seconds with one decimal, e.g. `12.5 s`.
  String _formatSeconds(int milliseconds) {
    final int tenthsOfSecond = (milliseconds + 50) ~/ 100;
    return '${tenthsOfSecond ~/ 10}.${tenthsOfSecond % 10} s';
  }
}

/// The unknown voices to tick for one saved person, each with Play to check it (#136).
final class _VoicePickerDialog extends StatefulWidget {
  const _VoicePickerDialog({
    required this.controller,
    required this.person,
    required this.titleOf,
    required this.detailsOf,
  });

  final VoiceGalleryController controller;
  final SavedPerson person;

  /// The voice's name as the gallery shows it.
  final String Function(UnknownSpeakerCluster) titleOf;

  /// Its meeting and length.
  final String Function(UnknownSpeakerCluster) detailsOf;

  @override
  State<_VoicePickerDialog> createState() => _VoicePickerDialogState();
}

final class _VoicePickerDialogState extends State<_VoicePickerDialog> {
  /// The ids of the voices ticked.
  final Set<String> _picked = <String>{};

  @override
  Widget build(BuildContext context) {
    final String name = widget.person.displayName;
    return AlertDialog(
      title: Text(l10n.addVoicesTo(name)),
      content: SizedBox(
        width: 480,
        child: AnimatedBuilder(
          animation: widget.controller,
          builder: (BuildContext context, Widget? child) => ListView(
            shrinkWrap: true,
            children: <Widget>[
              Text(l10n.tickTheVoicesThatAre(name)),
              const SizedBox(height: 8),
              for (final UnknownSpeakerCluster voice
                  in widget.controller.clusters)
                CheckboxListTile(
                  key: ValueKey<String>('pick-${voice.clusterId}'),
                  contentPadding: EdgeInsets.zero,
                  controlAffinity: ListTileControlAffinity.leading,
                  value: _picked.contains(voice.clusterId),
                  onChanged: (bool? ticked) => setState(() => ticked ?? false
                      ? _picked.add(voice.clusterId)
                      : _picked.remove(voice.clusterId)),
                  title: Text(widget.titleOf(voice)),
                  subtitle: Text(widget.detailsOf(voice)),
                  secondary: voice.hasReviewClip
                      ? TextButton(
                          key: ValueKey<String>('pick-play-${voice.clusterId}'),
                          onPressed: () => widget.controller.play(voice),
                          child: Text(widget.controller.playingClusterId ==
                                  voice.clusterId
                              ? l10n.stop
                              : l10n.play),
                        )
                      : null,
                ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          key: const ValueKey<String>('confirm-add-voices'),
          onPressed: _picked.isEmpty
              ? null
              : () => Navigator.of(context).pop(<UnknownSpeakerCluster>[
                    for (final UnknownSpeakerCluster voice
                        in widget.controller.clusters)
                      if (_picked.contains(voice.clusterId)) voice,
                  ]),
          child: Text(l10n.addTo(name)),
        ),
      ],
    );
  }
}
