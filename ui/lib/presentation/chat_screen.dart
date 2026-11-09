// The chat page (#97): ask the default AI connection about one meeting, one project, or every
// meeting, summary and project. Each answer says what the model was given and what was left out.
// The chat is kept on this device; it can be deleted from the menu.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/presentation/markdown_view.dart';

/// Finds the question field in tests.
const Key chatQuestionFieldKey = ValueKey<String>('chat-question');

/// Opens the chat page.
void openChat(BuildContext context,
    {required ChatBridge bridge,
    required ChatTopic topic,
    required String title}) {
  Navigator.of(context).push(MaterialPageRoute<void>(
    builder: (BuildContext context) =>
        ChatScreen(bridge: bridge, topic: topic, title: title),
  ));
}

/// Shows a chat and asks questions.
final class ChatScreen extends StatefulWidget {
  /// Creates the page.
  const ChatScreen(
      {required this.bridge,
      required this.topic,
      required this.title,
      super.key});

  final ChatBridge bridge;
  final ChatTopic topic;

  /// Such as `Chat: Budget review`.
  final String title;

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
      final (ChatEntry asked, ChatEntry answer) =
          await widget.bridge.ask(widget.topic, question);
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
        title: const Text('Delete this chat?'),
        content: const Text(
            'Its questions and answers are deleted from this device.'),
        actions: <Widget>[
          TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Cancel')),
          FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('Delete')),
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
      for (final ChatEntry message in _messages) _Bubble(message: message),
      if (_pending case final String question)
        _Bubble(
            message: ChatEntry(
                fromOwner: true, text: question, createdAt: DateTime.now())),
    ];
    return Scaffold(
      appBar: AppBar(
        title: Text(widget.title),
        actions: <Widget>[
          IconButton(
            tooltip: 'Delete this chat',
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
                                'Ask about your notes. The answer comes from your '
                                'default AI connection, which is sent the notes that fit.',
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
                    labelText: 'Your question',
                    border: const OutlineInputBorder(),
                    suffixIcon: IconButton(
                      tooltip: 'Ask',
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
  const _Bubble({required this.message});

  final ChatEntry message;

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
                if (!mine && message.included.isNotEmpty) ...<Widget>[
                  const SizedBox(height: 8),
                  Text('Given: ${message.included.join('; ')}', style: small),
                ],
                if (!mine && message.omitted.isNotEmpty)
                  Text('Left out for length: ${message.omitted.join('; ')}',
                      style: small),
                if (!mine && message.connection != null)
                  Text('Answered by ${message.connection}', style: small),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
