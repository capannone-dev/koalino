// The chat page (#97): ask the default AI connection about one meeting, one project, or every
// meeting, summary and project. On the phone, a paired computer answers by default (#107); the
// "Who answers" menu switches to the phone's own connection. Each answer says what the model was given and what was left out.
// The chat is kept on this device; it can be deleted from the menu. The places an answer cites
// are listed under it, and open the meeting at that moment or the file (#110).

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/markdown_view.dart';
import 'package:note_taker_ui/presentation/resource_viewer_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Opens a meeting's page at `atMs` (null for the top).
typedef ChatMeetingOpener = void Function(
    BuildContext context, String meetingId, int? atMs);

/// Finds the question field in tests.
const Key chatQuestionFieldKey = ValueKey<String>('chat-question');

/// Opens the chat page.
///
/// Inputs: The bridge, the topic and the page title; optionally how to open a cited meeting and
/// the projects controller that finds a cited file. A source without its opener is shown but
/// can't be opened. Side effects: Pushes a page.
void openChat(BuildContext context,
    {required ChatBridge bridge,
    required ChatTopic topic,
    required String title,
    ChatMeetingOpener? openMeeting,
    ProjectsController? projectsController}) {
  Navigator.of(context).push(MaterialPageRoute<void>(
    builder: (BuildContext context) => ChatScreen(
        bridge: bridge,
        topic: topic,
        title: title,
        openMeeting: openMeeting,
        projectsController: projectsController),
  ));
}

/// Shows a chat and asks questions.
final class ChatScreen extends StatefulWidget {
  /// Creates the page.
  const ChatScreen(
      {required this.bridge,
      required this.topic,
      required this.title,
      this.openMeeting,
      this.projectsController,
      super.key});

  final ChatBridge bridge;
  final ChatTopic topic;

  /// Such as `Chat: Budget review`.
  final String title;

  /// Opens a cited meeting; without it, meeting sources can't be opened.
  final ChatMeetingOpener? openMeeting;

  /// Finds a cited file; without it, file sources can't be opened.
  final ProjectsController? projectsController;

  @override
  State<ChatScreen> createState() => _ChatScreenState();
}

/// Keeps the messages, the question being typed and the status.
final class _ChatScreenState extends State<ChatScreen> {
  final TextEditingController _question = TextEditingController();
  final ScrollController _scroll = ScrollController();
  List<ChatEntry> _messages = <ChatEntry>[];
  String? _error;
  bool _loading = true;
  bool _asking = false;

  /// The question waiting for its answer, shown until it is kept.
  String? _pending;

  /// Whether a paired computer answers (#107); the phone's default.
  late bool _onComputer = widget.bridge.canAskPairedComputer;

  @override
  void initState() {
    super.initState();
    _load();
  }

  @override
  void dispose() {
    _question.dispose();
    _scroll.dispose();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final List<ChatEntry> messages =
          await widget.bridge.history(widget.topic);
      if (mounted) {
        setState(() {
          _messages = messages;
          _loading = false;
        });
        _toEnd();
      }
    } on ChatException catch (error) {
      if (mounted) {
        setState(() {
          _error = error.message;
          _loading = false;
        });
      }
    }
  }

  /// Asks the typed question. Side effects: One core call, which waits for the model.
  Future<void> _ask() async {
    final String question = _question.text.trim();
    if (question.isEmpty || _asking) {
      return;
    }
    setState(() {
      _asking = true;
      _pending = question;
      _error = null;
    });
    _toEnd();
    try {
      final (ChatEntry asked, ChatEntry answer) = await widget.bridge
          .ask(widget.topic, question, onPairedComputer: _onComputer);
      if (!mounted) {
        return;
      }
      setState(() {
        _messages = <ChatEntry>[..._messages, asked, answer];
        _question.clear();
      });
    } on ChatException catch (error) {
      if (mounted) {
        setState(() => _error = error.message);
      }
    } finally {
      if (mounted) {
        setState(() {
          _asking = false;
          _pending = null;
        });
        _toEnd();
      }
    }
  }

  Future<void> _clear() async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.deleteThisChat),
        content: Text(l10n.itsQuestionsAndAnswersAreDeleted),
        actions: <Widget>[
          TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.cancel)),
          FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.delete)),
        ],
      ),
    );
    if (confirmed != true) {
      return;
    }
    try {
      await widget.bridge.clear(widget.topic);
      if (mounted) {
        setState(() => _messages = <ChatEntry>[]);
      }
    } on ChatException catch (error) {
      if (mounted) {
        setState(() => _error = error.message);
      }
    }
  }

  /// Opens a cited place: the meeting at its moment, or the file in its viewer.
  ///
  /// Side effects: May open the file's project in the controller, and pushes a page. A file that
  /// is gone shows a message.
  Future<void> _openSource(ChatSource source) async {
    if (source.meetingId case final String meetingId) {
      widget.openMeeting?.call(context, meetingId, source.atMs);
      return;
    }
    final ProjectsController? projects = widget.projectsController;
    final String? projectId = source.projectId;
    final String? resourceId = source.resourceId;
    if (projects == null || projectId == null || resourceId == null) {
      return;
    }
    final NavigatorState navigator = Navigator.of(context);
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    if (projects.openProjectId != projectId || projects.openDetail == null) {
      await projects.openProject(projectId);
    }
    final ProjectResource? resource = projects.openDetail?.resources
        .where(
            (ProjectResource resource) => resource.attachmentId == resourceId)
        .firstOrNull;
    if (!navigator.mounted) {
      return;
    }
    if (resource == null) {
      messenger
          .showSnackBar(SnackBar(content: Text(l10n.thatFileIsNoLongerIn)));
      return;
    }
    await navigator.push(MaterialPageRoute<void>(
      builder: (BuildContext context) => ResourceViewerScreen(
          controller: projects, projectId: projectId, resource: resource),
    ));
  }

  /// Whether a source can be opened from this page.
  bool _canOpen(ChatSource source) => source.meetingId != null
      ? widget.openMeeting != null
      : widget.projectsController != null &&
          source.projectId != null &&
          source.resourceId != null;

  void _toEnd() {
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (_scroll.hasClients) {
        _scroll.jumpTo(_scroll.position.maxScrollExtent);
      }
    });
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final List<Widget> bubbles = <Widget>[
      for (final ChatEntry message in _messages)
        _Bubble(
            message: message,
            onOpenSource: (ChatSource source) =>
                _canOpen(source) ? () => _openSource(source) : null),
      if (_pending case final String question)
        _Bubble(
            message: ChatEntry(
                fromOwner: true, text: question, createdAt: DateTime.now())),
    ];
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.title),
        actions: <Widget>[
          if (widget.bridge.canAskPairedComputer)
            PopupMenuButton<bool>(
              tooltip: l10n.whoAnswers,
              icon: Icon(_onComputer
                  ? Icons.computer_outlined
                  : Icons.smartphone_outlined),
              enabled: !_asking,
              initialValue: _onComputer,
              onSelected: (bool onComputer) =>
                  setState(() => _onComputer = onComputer),
              itemBuilder: (BuildContext context) => <PopupMenuEntry<bool>>[
                CheckedPopupMenuItem<bool>(
                    value: true,
                    checked: _onComputer,
                    child: Text(l10n.myPairedComputer)),
                CheckedPopupMenuItem<bool>(
                    value: false,
                    checked: !_onComputer,
                    child: Text(l10n.thisPhoneSAiConnection)),
              ],
            ),
          IconButton(
            tooltip: l10n.deleteThisChat2,
            icon: const Icon(Icons.delete_sweep_outlined),
            onPressed: _messages.isEmpty || _asking ? null : _clear,
          ),
        ],
      ),
      body: Align(
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 840),
          child: Column(
            children: <Widget>[
              Expanded(
                child: _loading
                    ? const Center(child: CircularProgressIndicator())
                    : bubbles.isEmpty
                        ? Center(
                            child: Padding(
                              padding: const EdgeInsets.all(24),
                              child: Text(
                                _onComputer
                                    ? l10n.askAboutYourNotesAnswersCome
                                    : l10n.askAboutYourNotesTheAnswer,
                                textAlign: TextAlign.center,
                                style: theme.textTheme.bodyMedium?.copyWith(
                                    color: theme.colorScheme.onSurfaceVariant),
                              ),
                            ),
                          )
                        : ListView(
                            controller: _scroll,
                            padding: const EdgeInsets.all(12),
                            children: bubbles,
                          ),
              ),
              if (_asking) const LinearProgressIndicator(),
              if (_error case final String error)
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                  child: Semantics(
                    liveRegion: true,
                    child: Text(error,
                        style: TextStyle(color: theme.colorScheme.error)),
                  ),
                ),
              Padding(
                padding: const EdgeInsets.all(12),
                child: TextField(
                  key: chatQuestionFieldKey,
                  controller: _question,
                  enabled: !_loading,
                  minLines: 1,
                  maxLines: 5,
                  textInputAction: TextInputAction.send,
                  onSubmitted: (_) => _ask(),
                  decoration: InputDecoration(
                    labelText: l10n.yourQuestion,
                    border: const OutlineInputBorder(),
                    suffixIcon: IconButton(
                      tooltip: l10n.ask,
                      icon: const Icon(Icons.send),
                      onPressed: _asking ? null : _ask,
                    ),
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// One message: the owner's on the right, the model's on the left with what it was given.
final class _Bubble extends StatelessWidget {
  const _Bubble({required this.message, this.onOpenSource});

  final ChatEntry message;

  /// What tapping a source does; null (or a null result) shows it without a link.
  final VoidCallback? Function(ChatSource source)? onOpenSource;

  /// Such as `[1] Meeting "Review" · 1:05`.
  static String sourceLabel(ChatSource source) => <String>[
        '[${source.number}] ${source.label}',
        if (source.atMs case final int atMs) formatMeetingDuration(atMs),
      ].join(' · ');

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final bool mine = message.fromOwner;
    final TextStyle? small = theme.textTheme.bodySmall
        ?.copyWith(color: theme.colorScheme.onSurfaceVariant);
    return Align(
      alignment: mine ? Alignment.centerRight : Alignment.centerLeft,
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 680),
        child: Card(
          color: mine
              ? theme.colorScheme.primaryContainer
              : theme.colorScheme.surfaceContainerHigh,
          child: Padding(
            padding: const EdgeInsets.all(12),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                if (mine)
                  SelectableText(message.text)
                else
                  MarkdownView(markdownText: message.text, embedded: true),
                if (!mine && message.sources.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  Wrap(
                    spacing: 8,
                    runSpacing: 4,
                    children: <Widget>[
                      for (final ChatSource source in message.sources)
                        switch (onOpenSource?.call(source)) {
                          final VoidCallback open => ActionChip(
                              avatar: Icon(source.meetingId != null
                                  ? Icons.event_note_outlined
                                  : Icons.description_outlined),
                              label: Text(sourceLabel(source)),
                              tooltip: l10n.open,
                              onPressed: open,
                            ),
                          null => Chip(label: Text(sourceLabel(source))),
                        },
                    ],
                  ),
                ],
                if (!mine && message.included.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  Text(l10n.given(message.included.join('; ')), style: small),
                ],
                if (!mine && message.omitted.isNotEmpty)
                  Text(l10n.leftOutForLength(message.omitted.join('; ')),
                      style: small),
                if (message.connection case final String connection when !mine)
                  Text(l10n.answeredBy(connection), style: small),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
