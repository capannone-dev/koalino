// A meeting's summary (#95): the page that shows it and writes it with a chosen connection, the
// summary button shown beside meeting names, and the menu opened from a meeting row (Summarize
// with ▸ a connection, the meeting's own prompt, back to the Settings prompt). The model runs in
// the core on a background isolate, so leaving the page does not stop it. A phone can ask a
// paired computer to write the summary instead (#107); the page then shows how far it has got.

import 'dart:async';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/controllers/handoff_controller.dart';
import 'package:note_taker_ui/presentation/handoff_widgets.dart';
import 'package:note_taker_ui/presentation/markdown_view.dart';
import 'package:note_taker_ui/presentation/meeting_edit_widgets.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

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

/// The summary button beside a meeting's name: an icon with the word "Summary", since the icon
/// alone was not self-explanatory (owner, 2026-11-15). It opens the summary page, and its icon
/// spins while one is being written.
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
        builder: (BuildContext context, Widget? child) => Tooltip(
          message: controller.isSummarizing(meetingId)
              ? l10n.writingTheSummaryOf(title)
              : l10n.summaryOf(title),
          child: TextButton.icon(
            key: ValueKey<String>('summary-$meetingId'),
            style: TextButton.styleFrom(
              visualDensity: VisualDensity.compact,
              padding: const EdgeInsets.symmetric(horizontal: 8),
            ),
            icon: controller.isSummarizing(meetingId)
                ? const SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Icon(Icons.summarize_outlined),
            label: Text(l10n.summary),
            onPressed: () => openMeetingSummary(context,
                controller: controller, meetingId: meetingId, title: title),
          ),
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

/// Hands the meeting to the owner's own AI app (#114).
final class _UseOwnApp extends _SummaryMenuChoice {
  const _UseOwnApp();
}

/// Asks a paired computer to write the summary (#107).
final class _AskComputer extends _SummaryMenuChoice {
  const _AskComputer();
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
      title: Text(l10n.summaryOf(meeting.title)),
      children: <Widget>[
        if (controller.connections.isEmpty)
          ListTile(
            leading: Icon(Icons.info_outline),
            title: Text(l10n.addAnAiConnectionInSettings2),
          ),
        for (final LlmConnection connection in controller.connections)
          SimpleDialogOption(
            onPressed: () =>
                Navigator.of(context).pop(_SummarizeWith(connection.id)),
            child: ListTile(
              leading: const Icon(Icons.auto_awesome_outlined),
              title: Text(l10n.summarizeWith(connection.label)),
              subtitle: Text(connection.isDefault
                  ? l10n.defaultLabel2(connection.model)
                  : connection.model),
            ),
          ),
        if (controller.asksPairedComputer)
          SimpleDialogOption(
            onPressed: () => Navigator.of(context).pop(const _AskComputer()),
            child: ListTile(
              leading: Icon(Icons.computer_outlined),
              title: Text(l10n.summarizeOnMyComputer),
              subtitle: Text(l10n.withClaudeCodeCodexOrThe),
            ),
          ),
        if (controller.handoff != null)
          SimpleDialogOption(
            onPressed: () => Navigator.of(context).pop(const _UseOwnApp()),
            child: ListTile(
              leading: Icon(Icons.open_in_new),
              title: Text(l10n.useYourOwnAiApp2),
              subtitle: Text(controller.handoff?.viaBrowser ?? false
                  ? l10n.claudeAiOrChatgptComInYourBrowser
                  : l10n.claudeChatgptOrAnotherAppWith),
            ),
          ),
        const Divider(),
        SimpleDialogOption(
          onPressed: () => Navigator.of(context).pop(const _OpenSummary()),
          child: ListTile(
            leading: Icon(Icons.summarize_outlined),
            title: Text(l10n.openSummary),
          ),
        ),
        SimpleDialogOption(
          onPressed: () => Navigator.of(context).pop(const _EditPrompt()),
          child: ListTile(
            leading: Icon(Icons.edit_note),
            title: Text(l10n.editPromptForThisMeeting),
          ),
        ),
        if (state?.meetingPrompt != null)
          SimpleDialogOption(
            onPressed: () => Navigator.of(context).pop(const _ResetPrompt()),
            child: ListTile(
              leading: Icon(Icons.restore),
              title: Text(l10n.useTheSettingsPrompt),
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
    case _AskComputer():
      await controller.askComputer(meeting.meetingId);
      if (context.mounted) {
        await openMeetingSummary(context,
            controller: controller,
            meetingId: meeting.meetingId,
            title: meeting.title);
      }
    case _UseOwnApp():
      if (controller.handoff case final HandoffController handoff) {
        await showHandoffSheet(context,
            controller: handoff,
            meetingId: meeting.meetingId,
            title: meeting.title);
      }
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
        title: Text(l10n.promptForThisMeeting),
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
                decoration: InputDecoration(
                  labelText: l10n.promptForThisMeeting,
                  border: OutlineInputBorder(),
                ),
              ),
              if (widget.placeholders.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(
                    l10n.placeholders(widget.placeholders
                        .map((String name) => '{{$name}}')
                        .join(' ')),
                    style: Theme.of(context).textTheme.bodySmall,
                  ),
                ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(_field.text),
            child: Text(l10n.savePrompt),
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

/// How often an open request to a paired computer is read again while the page is shown.
const Duration _pairedRequestRefresh = Duration(seconds: 10);

/// Keeps the chosen connection, and reads a request to a paired computer again while it is open.
final class _MeetingSummaryScreenState extends State<MeetingSummaryScreen> {
  String? _connectionId;
  Timer? _refresh;

  @override
  void initState() {
    super.initState();
    if (widget.controller.settings == null) {
      widget.controller.load();
    }
    widget.controller.loadMeeting(widget.meetingId);
    if (widget.controller.asksPairedComputer) {
      _refresh = Timer.periodic(_pairedRequestRefresh, (_) {
        final PairedRequestState? state =
            widget.controller.stateFor(widget.meetingId)?.request?.state;
        if (state == PairedRequestState.waiting ||
            state == PairedRequestState.working) {
          widget.controller.loadMeeting(widget.meetingId);
        }
      });
    }
  }

  @override
  void dispose() {
    _refresh?.cancel();
    super.dispose();
  }

  /// Opens the Markdown editor on `text` and saves the result as the summary (#96).
  Future<void> _editText(String text) => showMarkdownEditor(
        context,
        title: l10n.summary,
        initialText: text,
        onSave: (String edited) =>
            widget.controller.editSummary(widget.meetingId, edited),
      );

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(title: Text(l10n.summary)),
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
                  if (controller.asksPairedComputer) ...<Widget>[
                    _PairedComputerPanel(
                      request: state?.request,
                      onAsk: () => controller.askComputer(widget.meetingId),
                      onCancel: () =>
                          controller.cancelComputer(widget.meetingId),
                    ),
                    const SizedBox(height: 12),
                  ],
                  if (connections.isEmpty)
                    if (!controller.asksPairedComputer)
                      Text(l10n.addAnAiConnectionInSettings3)
                    else
                      const SizedBox.shrink()
                  else
                    Wrap(
                      spacing: 12,
                      runSpacing: 12,
                      crossAxisAlignment: WrapCrossAlignment.center,
                      children: <Widget>[
                        DropdownMenu<String>(
                          key: summaryConnectionSelectorKey,
                          label: Text(l10n.summarizeWith2),
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
                              ? l10n.summarize
                              : l10n.summarizeAgain),
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
                        label: Text(l10n.editPromptForThisMeeting2),
                        onPressed: () => editMeetingPrompt(context,
                            controller: controller,
                            meetingId: widget.meetingId),
                      ),
                      if (state?.meetingPrompt != null)
                        TextButton.icon(
                          icon: const Icon(Icons.restore),
                          label: Text(l10n.useTheSettingsPrompt),
                          onPressed: () => controller.setMeetingPrompt(
                              widget.meetingId, null),
                        ),
                      if (controller.handoff
                          case final HandoffController handoff)
                        TextButton.icon(
                          icon: const Icon(Icons.open_in_new),
                          label: Text(l10n.useYourOwnAiApp),
                          onPressed: () => showHandoffSheet(context,
                              controller: handoff,
                              meetingId: widget.meetingId,
                              title: widget.title),
                        ),
                    ],
                  ),
                  if (writing) ...<Widget>[
                    const SizedBox(height: 12),
                    const LinearProgressIndicator(),
                    const SizedBox(height: 8),
                    Text(l10n.writingTheSummaryALocalModel),
                  ],
                  if (controller.meetingMessage(widget.meetingId)
                      case final String message) ...<Widget>[
                    const SizedBox(height: 12),
                    Text(message, style: theme.textTheme.bodyMedium),
                  ],
                  const Divider(height: 32),
                  if (summary == null) ...<Widget>[
                    Text(state == null ? l10n.loading : l10n.noSummaryYet),
                    if (state != null)
                      Align(
                        alignment: Alignment.centerLeft,
                        child: TextButton.icon(
                          icon: const Icon(Icons.edit_outlined),
                          label: Text(l10n.writeASummary),
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
                                ? l10n.writtenByYouOn(
                                    formatMeetingCreatedAt(summary.createdAt))
                                : l10n.writtenByOn(
                                    summary.model,
                                    summary.connectionLabel,
                                    formatMeetingCreatedAt(summary.createdAt),
                                    summary.edited ? ' · edited' : ''),
                            style: theme.textTheme.bodySmall?.copyWith(
                                color: theme.colorScheme.onSurfaceVariant),
                          ),
                        ),
                        TextButton.icon(
                          icon: const Icon(Icons.edit_outlined),
                          label: Text(l10n.editSummary),
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

/// Asks a paired computer to write the summary (#107) and shows how far it has got.
final class _PairedComputerPanel extends StatelessWidget {
  const _PairedComputerPanel(
      {required this.request, required this.onAsk, required this.onCancel});

  /// The open request, or null when none was made.
  final PairedSummaryRequest? request;
  final VoidCallback onAsk;
  final VoidCallback onCancel;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final PairedSummaryRequest? request = this.request;
    final bool open =
        request != null && request.state != PairedRequestState.failed;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        if (!open)
          FilledButton.tonalIcon(
            icon: const Icon(Icons.computer_outlined),
            label: Text(request == null
                ? l10n.summarizeOnMyComputer
                : l10n.tryAgainOnMyComputer),
            onPressed: onAsk,
          ),
        if (request != null) ...<Widget>[
          if (request.state == PairedRequestState.working) ...<Widget>[
            const SizedBox(height: 8),
            const LinearProgressIndicator(),
          ],
          const SizedBox(height: 8),
          Text(describePairedRequest(request),
              style: theme.textTheme.bodyMedium),
          if (open)
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton(
                onPressed: onCancel,
                child: Text(l10n.cancelRequest),
              ),
            ),
        ],
      ],
    );
  }
}
