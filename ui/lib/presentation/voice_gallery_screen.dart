// Renders a meeting's unknown-speaker clusters with sample playback and name assignment.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';

/// Displays unknown-speaker clusters for one meeting using controller-owned state.
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
          if (controller.isLoading && controller.clusters.isEmpty) {
            return const Center(child: CircularProgressIndicator());
          }
          if (controller.errorMessage case final String errorMessage) {
            return Center(child: Text(errorMessage));
          }
          _pruneNameControllers(controller.clusters);
          final String? statusMessage = controller.statusMessage;
          if (controller.clusters.isEmpty) {
            return Center(
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: <Widget>[
                  if (statusMessage != null) Text(statusMessage),
                  const Text('No unknown speakers to review'),
                ],
              ),
            );
          }
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
              for (final (int index, UnknownSpeakerCluster cluster)
                  in controller.clusters.indexed)
                _buildClusterCard(context, controller, index, cluster),
            ],
          );
        },
      ),
    );
  }

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
              subtitle: Text(_formatSummary(cluster)),
              trailing: cluster.reviewClipPath == null
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
                    decoration: const InputDecoration(
                      labelText: 'Name',
                      border: OutlineInputBorder(),
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
    _nameControllers.removeWhere(
        (String clusterId, TextEditingController nameController) {
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
