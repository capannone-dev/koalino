// Renders the dialog that starts a meeting with no recording (#129): an optional title, then the
// meeting opens to be filled by writing notes or adding files.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Asks for a new meeting's title.
///
/// Inputs: A context below a `Navigator`. Outputs: The title as typed (possibly empty, for an
/// untitled meeting), or `null` on Cancel or Escape. Side effects: None; the caller creates the
/// meeting.
Future<String?> showNewMeetingDialog(BuildContext context) =>
    showDialog<String>(
      context: context,
      builder: (BuildContext context) => const NewMeetingDialog(),
    );

/// The dialog: what a meeting without a recording is for, and its title.
final class NewMeetingDialog extends StatefulWidget {
  /// Creates the dialog.
  const NewMeetingDialog({super.key});

  @override
  State<NewMeetingDialog> createState() => _NewMeetingDialogState();
}

/// Holds the title field.
final class _NewMeetingDialogState extends State<NewMeetingDialog> {
  final TextEditingController _title = TextEditingController();

  @override
  void dispose() {
    _title.dispose();
    super.dispose();
  }

  /// Closes the dialog with the typed title.
  void _create() => Navigator.of(context).pop(_title.text.trim());

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(l10n.newMeeting),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 400),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(l10n.newMeetingExplanation),
            const SizedBox(height: 16),
            TextField(
              controller: _title,
              autofocus: true,
              maxLength: 200,
              textInputAction: TextInputAction.done,
              decoration: InputDecoration(
                labelText: l10n.titleOptional,
                border: const OutlineInputBorder(),
              ),
              onSubmitted: (_) => _create(),
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(onPressed: _create, child: Text(l10n.create)),
      ],
    );
  }
}
