// Renders locally playable unassigned clips and weak-match confirmation prompts.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/voice_gallery_controller.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';

/// Displays review clips for one meeting using controller-owned application state.
final class VoiceGalleryScreen extends StatefulWidget {
  /// Creates a Voice Gallery bound to an injected controller.
  const VoiceGalleryScreen({required this.voiceGalleryController, super.key});

  /// Owns asynchronous loading and all FFI-bound user actions.
  final VoiceGalleryController voiceGalleryController;

  @override
  State<VoiceGalleryScreen> createState() => _VoiceGalleryScreenState();
}

/// Starts loading once and renders controller changes without native knowledge.
final class _VoiceGalleryScreenState extends State<VoiceGalleryScreen> {
  @override
  void initState() {
    super.initState();
    widget.voiceGalleryController.load();
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
          if (controller.isLoading && controller.clips.isEmpty) {
            return const Center(child: CircularProgressIndicator());
          }
          if (controller.errorMessage case final String errorMessage) {
            return Center(child: Text(errorMessage));
          }
          if (controller.clips.isEmpty) {
            return const Center(child: Text('No unassigned voice clips'));
          }
          return ListView.builder(
            itemCount: controller.clips.length,
            itemBuilder: (BuildContext context, int index) {
              final VoiceClip clip = controller.clips[index];
              return ListTile(
                title: Text(_formatRange(clip)),
                subtitle: clip.weakMatch == null
                    ? const Text('No suggested identity')
                    : const Text('Weak match requires review'),
                trailing: Wrap(
                  spacing: 8,
                  children: <Widget>[
                    TextButton(
                      onPressed: () => controller.play(clip),
                      child: const Text('Play'),
                    ),
                    if (clip.weakMatch != null)
                      FilledButton(
                        onPressed: () => _showWeakMatchDialog(context, clip),
                        child: const Text('Review weak match'),
                      ),
                    OutlinedButton(
                      onPressed: controller.identities.isEmpty
                          ? null
                          : () => _showExistingIdentityDialog(context, clip),
                      child: const Text('Assign Existing Identity'),
                    ),
                    OutlinedButton(
                      onPressed: () => _showNewIdentityDialog(context, clip),
                      child: const Text('Create New Identity'),
                    ),
                  ],
                ),
              );
            },
          );
        },
      ),
    );
  }

  /// Presents the available identities in a dropdown for manual clip assignment.
  Future<void> _showExistingIdentityDialog(
      BuildContext context, VoiceClip clip) async {
    String selectedIdentityId =
        widget.voiceGalleryController.identities.first.identityId;
    await showDialog<void>(
      context: context,
      builder: (BuildContext dialogContext) => StatefulBuilder(
        builder: (BuildContext context, StateSetter setDialogState) =>
            AlertDialog(
          title: const Text('Assign Existing Identity'),
          content: DropdownButton<String>(
            value: selectedIdentityId,
            isExpanded: true,
            items: widget.voiceGalleryController.identities
                .map(
                  (VoiceIdentity identity) => DropdownMenuItem<String>(
                    value: identity.identityId,
                    child: Text(identity.displayName),
                  ),
                )
                .toList(growable: false),
            onChanged: (String? identityId) {
              if (identityId != null) {
                setDialogState(() => selectedIdentityId = identityId);
              }
            },
          ),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.pop(dialogContext),
              child: const Text('Cancel'),
            ),
            FilledButton(
              onPressed: () async {
                Navigator.pop(dialogContext);
                await widget.voiceGalleryController
                    .assignExistingIdentity(clip, selectedIdentityId);
              },
              child: const Text('Assign'),
            ),
          ],
        ),
      ),
    );
  }

  /// Shows confirmation for the application-provided weak candidate.
  Future<void> _showWeakMatchDialog(
      BuildContext context, VoiceClip clip) async {
    final WeakMatch weakMatch = clip.weakMatch!;
    final VoiceIdentity identity = widget.voiceGalleryController.identities
        .cast<VoiceIdentity?>()
        .firstWhere(
          (VoiceIdentity? candidate) =>
              candidate?.identityId == weakMatch.identityId,
          orElse: () => VoiceIdentity(
            identityId: weakMatch.identityId,
            displayName: 'the suggested speaker',
          ),
        )!;
    await showDialog<void>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('Weak voice match'),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text('Is this ${identity.displayName}?'),
            Text('${(weakMatch.confidenceScore * 100).round()}% confidence'),
          ],
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('No'),
          ),
          FilledButton(
            onPressed: () async {
              Navigator.pop(dialogContext);
              await widget.voiceGalleryController
                  .confirmWeakMatch(clip, identity.identityId);
            },
            child: const Text('Confirm'),
          ),
        ],
      ),
    );
  }

  /// Prompts for the display name required to create an identity from a review clip.
  Future<void> _showNewIdentityDialog(
      BuildContext context, VoiceClip clip) async {
    final TextEditingController nameController = TextEditingController();
    await showDialog<void>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: const Text('Create New Identity'),
        content: TextField(controller: nameController, autofocus: true),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.pop(dialogContext),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () async {
              final String displayName = nameController.text.trim();
              if (displayName.isEmpty) {
                return;
              }
              Navigator.pop(dialogContext);
              await widget.voiceGalleryController
                  .createNewIdentity(clip, displayName);
            },
            child: const Text('Create'),
          ),
        ],
      ),
    );
    nameController.dispose();
  }

  /// Formats a segment range as seconds for a compact review list label.
  String _formatRange(VoiceClip clip) =>
      '${(clip.startTimeMs / 1000).toStringAsFixed(2)}s – ${(clip.endTimeMs / 1000).toStringAsFixed(2)}s';
}
