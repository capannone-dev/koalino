// Renders unknown voices with sample playback and name assignment, and the saved people. Opens
// from the Voices destination for all meetings, or from a meeting for that meeting's (Issue #63).
// Voices that sound like one person share a card and are named at once (#82).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/presentation/confirm_delete_dialog.dart';

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
      appBar: AppBar(title: const Text('Voice Gallery')),
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
                    child: const Text('Try again'),
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
              if (showsSavedPeople) _sectionTitle(context, 'Unknown voices'),
              if (controller.clusters.isEmpty)
                const Padding(
                  padding: EdgeInsets.symmetric(vertical: 8),
                  child: Text('No unknown speakers to review'),
                ),
              for (final (int index, UnknownVoiceGroup group)
                  in controller.groups.indexed)
                if (group.voices.length == 1)
                  _buildClusterCard(
                      context, controller, index, group.voices.single)
                else
                  _buildGroupCard(context, controller, index, group),
              if (showsSavedPeople) ...<Widget>[
                _sectionTitle(context, 'Saved people'),
                if (controller.savedPeople.isEmpty)
                  const Padding(
                    padding: EdgeInsets.symmetric(vertical: 8),
                    child: Text(
                        'No saved people yet. Name a voice to add someone.'),
                  ),
                for (final SavedPerson person in controller.savedPeople)
                  ListTile(
                    contentPadding: EdgeInsets.zero,
                    leading: const Icon(Icons.person_outline),
                    title: Text(person.displayName),
                    subtitle: Text(person.voiceVariantCount == 1
                        ? '1 voice sample'
                        : '${person.voiceVariantCount} voice samples'),
                    trailing: IconButton(
                      key: ValueKey<String>(
                          'delete-person-${person.identityId}'),
                      tooltip: 'Delete ${person.displayName}',
                      icon: const Icon(Icons.delete_outline),
                      onPressed: () =>
                          _confirmDeletePerson(context, controller, person),
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
        cluster.clusterId, TextEditingController.new);
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
              title: Text('Unknown speaker ${index + 1}'),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  if (isPlaying)
                    Text('Playing…',
                        style: TextStyle(
                            color: colors.onPrimaryContainer,
                            fontWeight: FontWeight.bold)),
                  Text(_formatSummary(cluster)),
                  if (controller.meetingId == null)
                    Text(cluster.meetingTitle ?? 'Untitled meeting'),
                ],
              ),
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (cluster.hasReviewClip)
                    TextButton(
                      key: ValueKey<String>('play-${cluster.clusterId}'),
                      onPressed: () => controller.play(cluster),
                      child: Text(isPlaying ? 'Stop' : 'Play'),
                    ),
                  IconButton(
                    key: ValueKey<String>('delete-${cluster.clusterId}'),
                    tooltip: 'Delete this voice',
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

  /// Builds the name field, with saved people to pick from, and the Assign button.
  ///
  /// `keyId` names the widgets' keys; `onAssign` receives the typed name.
  Widget _nameRow(
    VoiceGalleryController controller,
    TextEditingController nameController,
    String keyId,
    void Function(String) onAssign,
  ) {
    return Row(
      children: <Widget>[
        Expanded(
          child: TextField(
            key: ValueKey<String>('identity-name-$keyId'),
            controller: nameController,
            decoration: InputDecoration(
              labelText: 'Name',
              border: const OutlineInputBorder(),
              suffixIcon: controller.savedPeople.isEmpty
                  ? null
                  : PopupMenuButton<String>(
                      tooltip: 'Choose a saved person',
                      icon: const Icon(Icons.people_outline),
                      onSelected: (String name) => nameController.text = name,
                      itemBuilder: (BuildContext context) =>
                          <PopupMenuEntry<String>>[
                        for (final SavedPerson person in controller.savedPeople)
                          PopupMenuItem<String>(
                            value: person.displayName,
                            child: Text(person.displayName),
                          ),
                      ],
                    ),
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
          child: const Text('Assign'),
        ),
      ],
    );
  }

  /// Builds one card for several voices of one person (#82): their total, the meetings they
  /// come from, Play for the longest, the parts to leave out or play, and one name for all.
  Widget _buildGroupCard(
    BuildContext context,
    VoiceGalleryController controller,
    int index,
    UnknownVoiceGroup group,
  ) {
    final TextEditingController nameController =
        _nameControllers.putIfAbsent(group.groupId, TextEditingController.new);
    final bool isPlaying = group.voices.any((UnknownSpeakerCluster voice) =>
        voice.clusterId == controller.playingClusterId);
    final bool canPlay =
        group.voices.any((UnknownSpeakerCluster voice) => voice.hasReviewClip);
    final ColorScheme colors = Theme.of(context).colorScheme;
    final int meetingCount = group.meetingIds.length;
    final String partsLabel = '${group.voices.length} parts';
    final String coverage = meetingCount == 1
        ? partsLabel
        : '$partsLabel in $meetingCount meetings';
    final List<String> meetingTitles = <String>{
      for (final UnknownSpeakerCluster voice in group.voices)
        voice.meetingTitle ?? 'Untitled meeting',
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
              title: Text('Unknown speaker ${index + 1}'),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  if (isPlaying)
                    Text('Playing…',
                        style: TextStyle(
                            color: colors.onPrimaryContainer,
                            fontWeight: FontWeight.bold)),
                  Text('${_formatSeconds(group.totalDurationMs)} · $coverage'),
                  if (controller.meetingId == null)
                    Text(meetingTitles.join(', ')),
                ],
              ),
              trailing: Row(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (canPlay)
                    TextButton(
                      key: ValueKey<String>('play-group-${group.groupId}'),
                      onPressed: () => controller.playGroup(group),
                      child: Text(isPlaying ? 'Stop' : 'Play'),
                    ),
                  IconButton(
                    key: ValueKey<String>('delete-${group.groupId}'),
                    tooltip: 'Delete these voices',
                    icon: const Icon(Icons.delete_outline),
                    onPressed: () =>
                        _confirmDeleteGroup(context, controller, index, group),
                  ),
                ],
              ),
            ),
            ExpansionTile(
              key: ValueKey<String>('parts-${group.groupId}'),
              tilePadding: EdgeInsets.zero,
              title: Text(partsLabel),
              subtitle: const Text('These sound like one person. '
                  'Untick any that is someone else.'),
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
            '${voice.meetingTitle ?? 'Untitled meeting'} · '
            '${_formatSeconds(voice.totalDurationMs)}',
          ),
        ),
        if (voice.hasReviewClip)
          TextButton(
            key: ValueKey<String>('play-${voice.clusterId}'),
            onPressed: () => controller.play(voice),
            child: Text(isPlaying ? 'Stop' : 'Play'),
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
      title: 'Delete Unknown speaker ${index + 1}?',
      message: 'Its ${group.voices.length} voice samples go for good, here '
          'and on your paired devices. The transcripts stay.',
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
      title: 'Delete Unknown speaker ${index + 1}?',
      message: 'Its voice sample goes for good, here and on your paired '
          'devices. The transcript stays.',
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
      title: 'Delete ${person.displayName}?',
      message: 'Their name and voice samples go for good, here and on your '
          'paired devices. Transcripts stay and show an unknown speaker.',
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

  /// Formats total speech time (seconds, one decimal) and chunk count, e.g. `12.5 s · 3 chunks`.
  String _formatSummary(UnknownSpeakerCluster cluster) {
    final String chunkLabel = cluster.chunkCount == 1 ? 'chunk' : 'chunks';
    return '${_formatSeconds(cluster.totalDurationMs)} · '
        '${cluster.chunkCount} $chunkLabel';
  }

  /// Formats milliseconds as seconds with one decimal, e.g. `12.5 s`.
  String _formatSeconds(int milliseconds) {
    final int tenthsOfSecond = (milliseconds + 50) ~/ 100;
    return '${tenthsOfSecond ~/ 10}.${tenthsOfSecond % 10} s';
  }
}
