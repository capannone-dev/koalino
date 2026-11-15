// The owner's editing tools in the app (#96): a Markdown editor page with a preview, a meeting's
// notes section, and a wrapper that lets a transcript segment be corrected by a long press, a
// right-click, or the screen reader's "Edit text" action. Edits are saved by the core; these
// widgets only collect the text and show what came back.

import 'package:flutter/material.dart';
import 'package:flutter/semantics.dart';
import 'package:note_taker_ui/controllers/meeting_edit_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/markdown_view.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Key of the Markdown editor's text field, for tests.
const Key markdownEditorFieldKey = ValueKey<String>('markdown-editor-field');

/// Key of the segment editor's text field, for tests.
const Key segmentEditFieldKey = ValueKey<String>('segment-edit-field');

/// Saves edited text: `null` once saved, or a sentence saying why not.
typedef SaveText = Future<String?> Function(String text);

/// Opens the Markdown editor as a page of its own.
///
/// Inputs: The page title, the text to start from, and `onSave`.
/// Outputs: Whether the text was saved. Side effects: Whatever `onSave` does.
Future<bool> showMarkdownEditor(
  BuildContext context, {
  required String title,
  required String initialText,
  required SaveText onSave,
}) async =>
    await Navigator.of(context).push<bool>(MaterialPageRoute<bool>(
      builder: (BuildContext context) => MarkdownEditorScreen(
          title: title, initialText: initialText, onSave: onSave),
    )) ??
    false;

/// Edits Markdown text, with a preview as it will be shown.
final class MarkdownEditorScreen extends StatefulWidget {
  /// Creates the editor.
  const MarkdownEditorScreen({
    required this.title,
    required this.initialText,
    required this.onSave,
    super.key,
  });

  final String title;
  final String initialText;
  final SaveText onSave;

  @override
  State<MarkdownEditorScreen> createState() => _MarkdownEditorScreenState();
}

/// Keeps the text, the chosen view and the save state.
final class _MarkdownEditorScreenState extends State<MarkdownEditorScreen> {
  late final TextEditingController _text =
      TextEditingController(text: widget.initialText);
  bool _preview = false;
  bool _saving = false;
  String? _error;

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  /// Saves the text and closes the page, or shows why it could not.
  Future<void> _save() async {
    setState(() {
      _saving = true;
      _error = null;
    });
    final String? error = await widget.onSave(_text.text);
    if (!mounted) {
      return;
    }
    if (error == null) {
      Navigator.of(context).pop(true);
      return;
    }
    setState(() {
      _saving = false;
      _error = error;
    });
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.title),
        actions: <Widget>[
          IconButton(
            tooltip: l10n.save,
            icon: const Icon(Icons.check),
            onPressed: _saving ? null : _save,
          ),
        ],
      ),
      body: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 840),
          child: Padding(
            padding: const EdgeInsets.all(16),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.stretch,
              children: <Widget>[
                SegmentedButton<bool>(
                  segments: <ButtonSegment<bool>>[
                    ButtonSegment<bool>(
                        value: false,
                        label: Text(l10n.edit),
                        icon: Icon(Icons.edit_outlined)),
                    ButtonSegment<bool>(
                        value: true,
                        label: Text(l10n.preview),
                        icon: Icon(Icons.visibility_outlined)),
                  ],
                  selected: <bool>{_preview},
                  onSelectionChanged: (Set<bool> chosen) =>
                      setState(() => _preview = chosen.single),
                ),
                if (_error case final String error) ...<Widget>[
                  const SizedBox(height: 8),
                  Text(error,
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: theme.colorScheme.error)),
                ],
                const SizedBox(height: 12),
                Expanded(
                  child: _preview
                      ? SingleChildScrollView(
                          child: MarkdownView(
                              markdownText: _text.text, embedded: true),
                        )
                      : TextField(
                          key: markdownEditorFieldKey,
                          controller: _text,
                          // The editor opens to be typed in at once.
                          autofocus: true,
                          maxLines: null,
                          expands: true,
                          textAlignVertical: TextAlignVertical.top,
                          keyboardType: TextInputType.multiline,
                          decoration: const InputDecoration(
                            border: OutlineInputBorder(),
                            labelText: 'Markdown',
                            alignLabelWithHint: true,
                          ),
                        ),
                ),
                if (_saving) const LinearProgressIndicator(),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Shows a meeting's notes with a button to write or edit them.
final class MeetingNotesSection extends StatefulWidget {
  /// Creates the section. Side effects: Loads the notes when first shown.
  const MeetingNotesSection({
    required this.controller,
    required this.meetingId,
    super.key,
  });

  final MeetingEditController controller;
  final String meetingId;

  @override
  State<MeetingNotesSection> createState() => _MeetingNotesSectionState();
}

/// Loads the notes when a meeting is shown.
final class _MeetingNotesSectionState extends State<MeetingNotesSection> {
  @override
  void initState() {
    super.initState();
    _loadIfNeeded();
  }

  @override
  void didUpdateWidget(MeetingNotesSection oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.meetingId != widget.meetingId) {
      _loadIfNeeded();
    }
  }

  /// Reads the notes again each time a meeting is shown, since a paired device may have changed
  /// them; the copy read earlier stays on screen meanwhile.
  void _loadIfNeeded() => widget.controller.loadNotes(widget.meetingId);

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return ListenableBuilder(
      listenable: widget.controller,
      builder: (BuildContext context, Widget? child) {
        final String? notes = widget.controller.notesFor(widget.meetingId);
        final String? error = widget.controller.loadErrorFor(widget.meetingId);
        final bool hasNotes = notes != null && notes.trim().isNotEmpty;
        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Row(
              children: <Widget>[
                Expanded(
                  child: Semantics(
                    header: true,
                    child: Text(l10n.notes, style: theme.textTheme.titleMedium),
                  ),
                ),
                TextButton.icon(
                  icon: const Icon(Icons.edit_note),
                  label: Text(hasNotes ? l10n.editNotes : l10n.writeNotes),
                  onPressed: notes == null && error == null
                      ? null
                      : () => showMarkdownEditor(
                            context,
                            title: l10n.notes,
                            initialText: notes ?? '',
                            onSave: (String text) => widget.controller
                                .saveNotes(widget.meetingId, text),
                          ),
                ),
              ],
            ),
            const SizedBox(height: 8),
            if (error != null)
              Text(error)
            else if (notes == null)
              Text(l10n.loading)
            else if (hasNotes)
              SelectionArea(
                  child: MarkdownView(markdownText: notes, embedded: true))
            else
              Text(l10n.noNotesYet,
                  style: theme.textTheme.bodyMedium
                      ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
          ],
        );
      },
    );
  }
}

/// Lets the owner correct one transcript segment: long press, right-click, or the "Edit text"
/// screen reader action opens a dialog with its text.
final class EditableSegment extends StatelessWidget {
  /// Creates the wrapper around `child`, the segment as shown.
  const EditableSegment({
    required this.controller,
    required this.meetings,
    required this.meetingId,
    required this.segment,
    required this.child,
    super.key,
  });

  final MeetingEditController controller;
  final MeetingListController meetings;
  final String meetingId;
  final MeetingTranscriptSegment segment;
  final Widget child;

  /// Asks for the new text, saves it, and says what happened.
  ///
  /// Side effects: Writes the meeting's segments through the controller and shows a snack bar.
  Future<void> _edit(BuildContext context) async {
    final ScaffoldMessengerState? messenger =
        ScaffoldMessenger.maybeOf(context);
    final String? text = await showDialog<String>(
      context: context,
      builder: (BuildContext context) => _SegmentEditDialog(
        label: l10n.textAt(formatMeetingDuration(segment.startTimeMs)),
        initialText: segment.text,
      ),
    );
    if (text == null || text == segment.text) {
      return;
    }
    final String? error =
        await controller.editSegment(meetings, meetingId, segment, text);
    messenger?.showSnackBar(SnackBar(
      content: Text(error ??
          (text.trim().isEmpty
              ? l10n.removedThisTranscriptWonTBe
              : l10n.savedThisTranscriptWonTBe)),
    ));
  }

  @override
  Widget build(BuildContext context) {
    return Semantics(
      customSemanticsActions: <CustomSemanticsAction, VoidCallback>{
        CustomSemanticsAction(label: l10n.editText): () => _edit(context),
      },
      child: GestureDetector(
        behavior: HitTestBehavior.opaque,
        onLongPress: () => _edit(context),
        onSecondaryTap: () => _edit(context),
        child: child,
      ),
    );
  }
}

/// Asks for a segment's new text; returns it, or null when cancelled.
final class _SegmentEditDialog extends StatefulWidget {
  const _SegmentEditDialog({required this.label, required this.initialText});

  final String label;
  final String initialText;

  @override
  State<_SegmentEditDialog> createState() => _SegmentEditDialogState();
}

final class _SegmentEditDialogState extends State<_SegmentEditDialog> {
  late final TextEditingController _text =
      TextEditingController(text: widget.initialText);

  @override
  void dispose() {
    _text.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(l10n.editTheTranscript),
      content: SizedBox(
        width: 560,
        child: TextField(
          key: segmentEditFieldKey,
          controller: _text,
          autofocus: true,
          minLines: 2,
          maxLines: 8,
          decoration: InputDecoration(
            labelText: widget.label,
            helperText: l10n.leaveItEmptyToRemoveThis,
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        TextButton(
          onPressed: () => Navigator.of(context).pop(_text.text),
          child: Text(l10n.save),
        ),
      ],
    );
  }
}
