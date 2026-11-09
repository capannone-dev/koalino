// A meeting's summary (#95): the page that shows it and writes it with a chosen connection, the
// summary button shown beside meeting names, and the menu opened from a meeting row (Summarize
// with ▸ a connection, the meeting's own prompt, back to the Settings prompt). The model runs in
// the core on a background isolate, so leaving the page does not stop it.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/markdown_view.dart';
import 'package:note_taker_ui/presentation/meeting_edit_widgets.dart';

/// Finds the "Summarize with" selector in tests.
const Key summaryConnectionSelectorKey = ValueKey<String>('summary-connection');

/// Finds the meeting prompt field in tests.
const Key meetingPromptFieldKey = ValueKey<String>('meeting-prompt');

/// Opens a meeting's summary page.
///
/// Inputs: A context under a Navigator, the controller, and the meeting's ID and title.
/// Side effects: Pushes the page.
Future<void> openMeetingSummary(
  BuildContext context, {
  required SummariesController controller,
  required String meetingId,
  required String title,
}) =>
    Navigator.of(context).push(MaterialPageRoute<void>(
      builder: (BuildContext context) => MeetingSummaryScreen(
        controller: controller,
        meetingId: meetingId,
        title: title,
      ),
    ));

/// The summary icon beside a meeting's name; it opens the summary page, and spins while one is
/// being written.
final class MeetingSummaryButton extends StatelessWidget {
  /// Creates the button for one meeting.
  const MeetingSummaryButton({
    required this.controller,
    required this.meetingId,
    required this.title,
    super.key,
  });

  final SummariesController controller;
  final String meetingId;
  final String title;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: controller,
        builder: (BuildContext context, Widget? child) => IconButton(
          key: ValueKey<String>('summary-$meetingId'),
          tooltip: controller.isSummarizing(meetingId)
              ? 'Writing the summary of $title'
              : 'Summary of $title',
          icon: controller.isSummarizing(meetingId)
              ? const SizedBox.square(
                  dimension: 20,
                  child: CircularProgressIndicator(strokeWidth: 2),
                )
              : const Icon(Icons.summarize_outlined),
          onPressed: () => openMeetingSummary(context,
              controller: controller, meetingId: meetingId, title: title),
        ),
      );
}

/// What the summary menu can do.
sealed class _SummaryMenuChoice {
  const _SummaryMenuChoice();
}

/// Writes the summary with one connection.
final class _SummarizeWith extends _SummaryMenuChoice {
  const _SummarizeWith(this.connectionId);
  final String connectionId;
}

/// Opens the summary page.
final class _OpenSummary extends _SummaryMenuChoice {
  const _OpenSummary();
}

/// Edits the meeting's own prompt.
final class _EditPrompt extends _SummaryMenuChoice {
  const _EditPrompt();
}

/// Returns the meeting to the Settings prompt.
final class _ResetPrompt extends _SummaryMenuChoice {
  const _ResetPrompt();
}

/// Shows the summary menu of one meeting and runs the choice.
///
/// Inputs: A context, the controller and the meeting. Outputs: Completes when the menu and any
/// dialog close (a summary keeps being written afterwards). Side effects: May start a summary,
/// which sends the meeting to the chosen connection's server, or change the meeting's prompt.
Future<void> showMeetingSummaryMenu(
  BuildContext context, {
  required SummariesController controller,
  required MeetingSummary meeting,
}) async {
  if (controller.settings == null) {
    await controller.load();
  }
  await controller.loadMeeting(meeting.meetingId);
  if (!context.mounted) {
    return;
  }
  final MeetingSummaryState? state = controller.stateFor(meeting.meetingId);
  final _SummaryMenuChoice? choice = await showDialog<_SummaryMenuChoice>(
    context: context,
    builder: (BuildContext context) => SimpleDialog(
      title: Text('Summary of ${meeting.title}'),
      children: <Widget>[
        if (controller.connections.isEmpty)
          const ListTile(
            leading: Icon(Icons.info_outline),
            title: Text('Add an AI connection in Settings first.'),
          ),
        for (final LlmConnection connection in controller.connections)
          SimpleDialogOption(
            onPressed: () =>
                Navigator.of(context).pop(_SummarizeWith(connection.id)),
            child: ListTile(
              leading: const Icon(Icons.auto_awesome_outlined),
              title: Text('Summarize with ${connection.label}'),
              subtitle: Text(connection.isDefault
                  ? '${connection.model} · default'
                  : connection.model),
            ),
          ),
        const Divider(),
        SimpleDialogOption(
          onPressed: () => Navigator.of(context).pop(const _OpenSummary()),
          child: const ListTile(
            leading: Icon(Icons.summarize_outlined),
            title: Text('Open summary'),
          ),
        ),
        SimpleDialogOption(
          onPressed: () => Navigator.of(context).pop(const _EditPrompt()),
          child: const ListTile(
            leading: Icon(Icons.edit_note),
            title: Text('Edit prompt for this meeting…'),
          ),
        ),
        if (state?.meetingPrompt != null)
          SimpleDialogOption(
            onPressed: () => Navigator.of(context).pop(const _ResetPrompt()),
            child: const ListTile(
              leading: Icon(Icons.restore),
              title: Text('Use the Settings prompt'),
            ),
          ),
      ],
    ),
  );
  if (!context.mounted || choice == null) {
    return;
  }
  switch (choice) {
    case _SummarizeWith(:final String connectionId):
      final Future<void> writing =
          controller.summarize(meeting.meetingId, connectionId: connectionId);
      await openMeetingSummary(context,
          controller: controller,
          meetingId: meeting.meetingId,
          title: meeting.title);
      await writing;
    case _OpenSummary():
      await openMeetingSummary(context,
          controller: controller,
          meetingId: meeting.meetingId,
          title: meeting.title);
    case _EditPrompt():
      await editMeetingPrompt(context,
          controller: controller, meetingId: meeting.meetingId);
    case _ResetPrompt():
      await controller.setMeetingPrompt(meeting.meetingId, null);
  }
}

/// Opens the meeting's own prompt in a dialog, starting from the Settings prompt when it has
/// none.
///
/// Side effects: Saves the prompt in the meeting document when confirmed; an empty prompt
/// returns the meeting to the Settings prompt.
Future<void> editMeetingPrompt(
  BuildContext context, {
  required SummariesController controller,
  required String meetingId,
}) async {
  final String? prompt = await showDialog<String>(
    context: context,
    builder: (BuildContext context) => _MeetingPromptDialog(
      initial: controller.stateFor(meetingId)?.meetingPrompt ??
          controller.settings?.template ??
          '',
      placeholders: controller.settings?.placeholders ?? const <String>[],
    ),
  );
  if (prompt != null) {
    await controller.setMeetingPrompt(
        meetingId, prompt.trim().isEmpty ? null : prompt);
  }
}

/// The meeting prompt form; it owns its field so the field outlives the closing animation.
final class _MeetingPromptDialog extends StatefulWidget {
  const _MeetingPromptDialog(
      {required this.initial, required this.placeholders});

  final String initial;
  final List<String> placeholders;

  @override
  State<_MeetingPromptDialog> createState() => _MeetingPromptDialogState();
}

/// Holds the field.
final class _MeetingPromptDialogState extends State<_MeetingPromptDialog> {
  late final TextEditingController _field =
      TextEditingController(text: widget.initial);

  @override
  void dispose() {
    _field.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: const Text('Prompt for this meeting'),
        content: SizedBox(
          width: 560,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              TextField(
                key: meetingPromptFieldKey,
                controller: _field,
                minLines: 6,
                maxLines: 14,
                decoration: const InputDecoration(
                  labelText: 'Prompt for this meeting',
                  border: OutlineInputBorder(),
                ),
              ),
              if (widget.placeholders.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    'Placeholders: ${widget.placeholders.map((String name) => '{{$name}}').join(' ')}',
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(_field.text),
            child: const Text('Save prompt'),
          ),
        ],
      );
}

/// Shows a meeting's summary and writes it with a chosen connection.
final class MeetingSummaryScreen extends StatefulWidget {
  /// Creates the page.
  ///
  /// Inputs: The shared controller and the meeting. Side effects: Loads the settings (when not
  /// loaded yet) and the meeting's summary when first shown.
  const MeetingSummaryScreen({
    required this.controller,
    required this.meetingId,
    required this.title,
    super.key,
  });

  final SummariesController controller;
  final String meetingId;
  final String title;

  @override
  State<MeetingSummaryScreen> createState() => _MeetingSummaryScreenState();
}

/// Keeps the chosen connection.
final class _MeetingSummaryScreenState extends State<MeetingSummaryScreen> {
  String? _connectionId;

  @override
  void initState() {
    super.initState();
    if (widget.controller.settings == null) {
      widget.controller.load();
    }
    widget.controller.loadMeeting(widget.meetingId);
  }

  /// Opens the Markdown editor on `text` and saves the result as the summary (#96).
  Future<void> _editText(String text) => showMarkdownEditor(
        context,
        title: 'Summary',
        initialText: text,
        onSave: (String edited) =>
            widget.controller.editSummary(widget.meetingId, edited),
      );

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('Summary')),
      body: ListenableBuilder(
        listenable: widget.controller,
        builder: (BuildContext context, Widget? child) {
          final SummariesController controller = widget.controller;
          final MeetingSummaryState? state =
              controller.stateFor(widget.meetingId);
          final List<LlmConnection> connections = controller.connections;
          final String? chosen = connections.any(
                  (LlmConnection connection) => connection.id == _connectionId)
              ? _connectionId
              : controller.defaultConnection?.id ??
                  (connections.isEmpty ? null : connections.first.id);
          final bool writing = controller.isSummarizing(widget.meetingId);
          final StoredSummary? summary = state?.summary;
          return Align(
            alignment: Alignment.topCenter,
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 840),
              child: ListView(
                padding: const EdgeInsets.all(24),
                children: <Widget>[
                  Text(widget.title,
                      style: theme.textTheme.headlineSmall
                          ?.copyWith(fontWeight: FontWeight.bold)),
                  const SizedBox(height: 16),
                  if (connections.isEmpty)
                    const Text(
                        'Add an AI connection in Settings to write summaries.')
                  else
                    Wrap(
                      spacing: 12,
                      runSpacing: 12,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        DropdownMenu<String>(
                          key: summaryConnectionSelectorKey,
                          label: const Text('Summarize with'),
                          initialSelection: chosen,
                          enabled: !writing,
                          onSelected: (String? id) =>
                              setState(() => _connectionId = id),
                          dropdownMenuEntries: <DropdownMenuEntry<String>>[
                            for (final LlmConnection connection in connections)
                              DropdownMenuEntry<String>(
                                value: connection.id,
                                label: connection.label,
                              ),
                          ],
                        ),
                        FilledButton.icon(
                          icon: const Icon(Icons.auto_awesome_outlined),
                          label: Text(summary == null
                              ? 'Summarize'
                              : 'Summarize again'),
                          onPressed: writing
                              ? null
                              : () => controller.summarize(widget.meetingId,
                                  connectionId: chosen),
                        ),
                      ],
                    ),
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    children: <Widget>[
                      TextButton.icon(
                        icon: const Icon(Icons.edit_note),
                        label: const Text('Edit prompt for this meeting'),
                        onPressed: () => editMeetingPrompt(context,
                            controller: controller,
                            meetingId: widget.meetingId),
                      ),
                      if (state?.meetingPrompt != null)
                        TextButton.icon(
                          icon: const Icon(Icons.restore),
                          label: const Text('Use the Settings prompt'),
                          onPressed: () => controller.setMeetingPrompt(
                              widget.meetingId, null),
                        ),
                    ],
                  ),
                  if (writing) ...<Widget>[
                    const SizedBox(height: 12),
                    const LinearProgressIndicator(),
                    const SizedBox(height: 8),
                    const Text(
                        'Writing the summary… A local model can take a few minutes.'),
                  ],
                  if (controller.meetingMessage(widget.meetingId)
                      case final String message) ...<Widget>[
                    const SizedBox(height: 12),
                    Text(message, style: theme.textTheme.bodyMedium),
                  ],
                  const Divider(height: 32),
                  if (summary == null) ...<Widget>[
                    Text(state == null ? 'Loading…' : 'No summary yet.'),
                    if (state != null)
                      Align(
                        alignment: Alignment.centerLeft,
                        child: TextButton.icon(
                          icon: const Icon(Icons.edit_outlined),
                          label: const Text('Write a summary'),
                          onPressed: () => _editText(''),
                        ),
                      ),
                  ] else ...<Widget>[
                    Row(
                      children: <Widget>[
                        Expanded(
                          child: Text(
                            summary.connectionLabel.isEmpty &&
                                    summary.model.isEmpty
                                ? 'Written by you on '
                                    '${formatMeetingCreatedAt(summary.createdAt)}'
                                : 'Written by ${summary.model} (${summary.connectionLabel}) on '
                                    '${formatMeetingCreatedAt(summary.createdAt)}'
                                    '${summary.edited ? ' · edited' : ''}',
                            style: theme.textTheme.bodySmall?.copyWith(
                                color: theme.colorScheme.onSurfaceVariant),
                          ),
                        ),
                        TextButton.icon(
                          icon: const Icon(Icons.edit_outlined),
                          label: const Text('Edit summary'),
                          onPressed:
                              writing ? null : () => _editText(summary.text),
                        ),
                      ],
                    ),
                    const SizedBox(height: 12),
                    SelectionArea(
                        child: MarkdownView(
                            markdownText: summary.text, embedded: true)),
                  ],
                ],
              ),
            ),
          );
        },
      ),
    );
  }
}
