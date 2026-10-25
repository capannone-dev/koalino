// Renders Material 3 attachment import controls and user-confirmable meeting suggestions.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/attachment_controller.dart';

/// Displays the attachment-import affordance for a meeting-detail presentation surface.
final class AttachmentImportScreen extends StatelessWidget {
  /// Creates a native-free screen bound to an injected attachment workflow controller.
  const AttachmentImportScreen({required this.attachmentController, super.key});

  /// Owns picker, JSON parsing, and Rust confirmation state.
  final AttachmentController attachmentController;

  /// Builds the paperclip action and any controller-provided status text.
  ///
  /// Inputs: `context` supplies Material styling and modal routing.
  /// Outputs: A Material 3 attachment import surface.
  /// Side effects: Tapping the action can open a platform picker and a suggestion dialog.
  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(
        title: const Text('Meeting Details'),
        actions: <Widget>[
          AnimatedBuilder(
            animation: attachmentController,
            builder: (BuildContext context, Widget? child) => IconButton(
              tooltip: 'Add Attachment',
              onPressed: attachmentController.isLoading
                  ? null
                  : () => _pickAndShowSuggestions(context),
              icon: const Icon(Icons.attach_file),
            ),
          ),
        ],
      ),
      body: AnimatedBuilder(
        animation: attachmentController,
        builder: (BuildContext context, Widget? child) {
          if (attachmentController.errorMessage case final String errorMessage) {
            return Center(child: Text(errorMessage));
          }
          return const Center(child: Text('No attachments imported'));
        },
      ),
    );
  }

  /// Gets candidates from the controller and presents a modal only when Rust found matches.
  ///
  /// Inputs: `context` must remain mounted while picker and FFI work completes.
  /// Outputs: Completes after any suggestion dialog is dismissed.
  /// Side effects: Opens platform picker/native Rust workflow and may link an attachment.
  Future<void> _pickAndShowSuggestions(BuildContext context) async {
    final List<AttachmentSuggestion> suggestions =
        await attachmentController.pickAndLoadSuggestions();
    if (!context.mounted || suggestions.isEmpty) {
      return;
    }
    await showDialog<void>(
      context: context,
      builder: (BuildContext dialogContext) => _AttachmentSuggestionDialog(
        attachmentController: attachmentController,
        suggestions: suggestions,
      ),
    );
  }
}

/// Presents Rust-ranked candidates and makes the final attachment link explicitly user-approved.
final class _AttachmentSuggestionDialog extends StatelessWidget {
  /// Creates a dialog using already decoded candidates in Rust-provided ranking order.
  const _AttachmentSuggestionDialog({
    required this.attachmentController,
    required this.suggestions,
  });

  final AttachmentController attachmentController;
  final List<AttachmentSuggestion> suggestions;

  /// Builds the selection controls required before a meeting link can be persisted.
  ///
  /// Inputs: `context` supplies dialog routing and Material styling.
  /// Outputs: A modal list of confidence-ranked candidates.
  /// Side effects: Selecting a candidate calls Rust confirmation through the controller.
  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: const Text('Suggested meetings'),
      content: SizedBox(
        width: 420,
        child: ListView.builder(
          shrinkWrap: true,
          itemCount: suggestions.length,
          itemBuilder: (BuildContext context, int index) {
            final AttachmentSuggestion suggestion = suggestions[index];
            final int confidencePercent =
                (suggestion.confidenceScore * 100).round();
            return ListTile(
              title: Text('${suggestion.meetingId} ($confidencePercent% match)'),
              trailing: FilledButton(
                onPressed: () async {
                  await attachmentController.confirmSuggestion(suggestion);
                  if (context.mounted && attachmentController.errorMessage == null) {
                    Navigator.pop(context);
                  }
                },
                child: const Text('Select'),
              ),
            );
          },
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text('Choose Different Meeting'),
        ),
        TextButton(
          onPressed: () => Navigator.pop(context),
          child: const Text("Don't Link"),
        ),
      ],
    );
  }
}
