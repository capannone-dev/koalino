// Renders unknown voices with sample playback and name assignment, and the saved people. Opens
// from the Voices destination for all meetings, or from a meeting for that meeting's (Issue #63).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';

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
  /// Name-field controllers keyed by cluster ID, so typed text survives rebuilds.
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
          _pruneNameControllers(controller.clusters);
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
              for (final (int index, UnknownSpeakerCluster cluster)
                  in controller.clusters.indexed)
                _buildClusterCard(context, controller, index, cluster),
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
    return Card(
      margin: const EdgeInsets.only(bottom: 12),
      child: Padding(
        padding: const EdgeInsets.all(12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            ListTile(
              contentPadding: EdgeInsets.zero,
              title: Text('Unknown speaker ${index + 1}'),
              subtitle: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: <Widget>[
                  Text(_formatSummary(cluster)),
                  if (controller.meetingId == null)
                    Text(cluster.meetingTitle ?? 'Untitled meeting'),
                ],
              ),
              trailing: !cluster.hasReviewClip
                  ? null
                  : TextButton(
                      onPressed: () => controller.play(cluster),
                      child: const Text('Play'),
                    ),
            ),
            Row(
              children: <Widget>[
                Expanded(
                  child: TextField(
                    key: ValueKey<String>('identity-name-${cluster.clusterId}'),
                    controller: nameController,
                    decoration: InputDecoration(
                      labelText: 'Name',
                      border: const OutlineInputBorder(),
                      suffixIcon: controller.savedPeople.isEmpty
                          ? null
                          : PopupMenuButton<String>(
                              tooltip: 'Choose a saved person',
                              icon: const Icon(Icons.people_outline),
                              onSelected: (String name) =>
                                  nameController.text = name,
                              itemBuilder: (BuildContext context) =>
                                  <PopupMenuEntry<String>>[
                                for (final SavedPerson person
                                    in controller.savedPeople)
                                  PopupMenuItem<String>(
                                    value: person.displayName,
                                    child: Text(person.displayName),
                                  ),
                              ],
                            ),
                    ),
                    textInputAction: TextInputAction.done,
                    onSubmitted: (String typedName) =>
                        controller.assignCluster(cluster, typedName),
                  ),
                ),
                const SizedBox(width: 12),
                FilledButton(
                  key: ValueKey<String>('assign-${cluster.clusterId}'),
                  onPressed: controller.isLoading
                      ? null
                      : () => controller.assignCluster(
                          cluster, nameController.text),
                  child: const Text('Assign'),
                ),
              ],
            ),
          ],
        ),
      ),
    );
  }

  /// Forgets name fields whose clusters are no longer listed and disposes them after the frame,
  /// once their `TextField`s have unmounted.
  void _pruneNameControllers(List<UnknownSpeakerCluster> clusters) {
    final Set<String> listedClusterIds = clusters
        .map((UnknownSpeakerCluster cluster) => cluster.clusterId)
        .toSet();
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
    final int tenthsOfSecond = (cluster.totalDurationMs + 50) ~/ 100;
    final String chunkLabel = cluster.chunkCount == 1 ? 'chunk' : 'chunks';
    return '${tenthsOfSecond ~/ 10}.${tenthsOfSecond % 10} s · '
        '${cluster.chunkCount} $chunkLabel';
  }
}
