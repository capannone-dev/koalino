// Asks the owner to confirm an irreversible deletion, for voices and meetings alike.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';

/// Shows a dialog naming what will be deleted.
///
/// Inputs: A mounted context, the question, and what the deletion means.
/// Outputs: `true` only when the owner taps Delete; dismissing counts as no.
/// Side effects: Shows a dialog; deletes nothing itself.
Future<bool> confirmDelete(
  BuildContext context, {
  required String title,
  required String message,
}) async {
  final bool? confirmed = await showDialog<bool>(
    context: context,
    builder: (BuildContext dialogContext) => AlertDialog(
      title: Text(title),
      content: Text(message),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(false),
          child: const Text('Cancel'),
        ),
        FilledButton(
          style: FilledButton.styleFrom(
            backgroundColor: Theme.of(dialogContext).colorScheme.error,
            foregroundColor: Theme.of(dialogContext).colorScheme.onError,
          ),
          onPressed: () => Navigator.of(dialogContext).pop(true),
          child: const Text('Delete'),
        ),
      ],
    ),
  );
  return confirmed ?? false;
}
