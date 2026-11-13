// The AI assistant access page (#112), in Settings on desktops: lets the owner's own assistant
// read meetings and projects through the read-only MCP server, warns that the assistant may send
// what it reads to its online model, and shows how to connect Claude Code or another client.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:note_taker_ui/infrastructure/assistant_access_bridge.dart';

/// Copies text to the clipboard; replaced in tests.
typedef CopyText = Future<void> Function(String text);

/// Copies with the system clipboard.
Future<void> copyToClipboard(String text) =>
    Clipboard.setData(ClipboardData(text: text));

/// Shows and changes assistant access.
final class AssistantAccessScreen extends StatefulWidget {
  /// Creates the page.
  const AssistantAccessScreen(
      {required this.bridge, this.copyText = copyToClipboard, super.key});

  final AssistantAccessBridge bridge;
  final CopyText copyText;

  @override
  State<AssistantAccessScreen> createState() => _AssistantAccessScreenState();
}

/// Keeps the status and the error.
final class _AssistantAccessScreenState extends State<AssistantAccessScreen> {
  AssistantAccessStatus? _status;
  String? _error;
  bool _busy = true;

  @override
  void initState() {
    super.initState();
    _run(widget.bridge.status);
  }

  /// Runs a command and shows its status or error. Side effects: One core call.
  Future<void> _run(Future<AssistantAccessStatus> Function() command) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    AssistantAccessStatus? status;
    String? error;
    try {
      status = await command();
    } on AssistantAccessException catch (failure) {
      error = failure.message;
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _busy = false;
      _error = error;
      _status = status ?? _status;
    });
  }

  Future<void> _copy(String text, String what) async {
    await widget.copyText(text);
    if (mounted) {
      ScaffoldMessenger.of(context)
          .showSnackBar(SnackBar(content: Text('$what copied')));
    }
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final AssistantAccessStatus? status = _status;
    final bool installed = status?.serverPath != null;
    return Scaffold(
      appBar: AppBar(title: const Text('AI assistant access')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          const Text(
              'Let your own AI assistant, such as Claude Code, Codex or Claude Desktop, '
              'read your meetings and projects: titles, summaries, notes, transcripts and '
              'project files. It can only read; it cannot change or delete anything.'),
          const SizedBox(height: 12),
          Card(
            color: theme.colorScheme.errorContainer,
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Text(
                'What the assistant reads may be sent to its online model, under your '
                "account with that service. Turn this on only for an assistant you trust "
                'with your meetings.',
                style: TextStyle(color: theme.colorScheme.onErrorContainer),
              ),
            ),
          ),
          if (status != null && !installed)
            const Padding(
              padding: EdgeInsets.only(top: 12),
              child: Text(
                  'The assistant server is not installed with this copy of the app.'),
            ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Let my AI assistant read my notes'),
            value: status?.enabled ?? false,
            onChanged: _busy || !installed
                ? null
                : (bool enabled) =>
                    _run(() => widget.bridge.set(enabled: enabled)),
          ),
          if (_busy) const LinearProgressIndicator(),
          if (_error case final String error)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child:
                  Text(error, style: TextStyle(color: theme.colorScheme.error)),
            ),
          if (status != null && status.enabled && installed) ...<Widget>[
            const SizedBox(height: 12),
            Text('Connect your assistant', style: theme.textTheme.titleMedium),
            if (status.passphraseNeeded)
              const Padding(
                padding: EdgeInsets.only(top: 8),
                child: Text(
                    'Your notes are unlocked with a passphrase, which is never written into '
                    "an assistant's settings. The server reads them only when "
                    'NOTE_TAKER_ENCRYPTION_PASSPHRASE is set where the assistant starts it.'),
              ),
            if (status.claudeCode case final String command)
              _Snippet(
                title: 'Claude Code: run in a terminal',
                text: command,
                onCopy: () => _copy(command, 'Command'),
              ),
            if (status.config case final String config)
              _Snippet(
                title:
                    'Claude Desktop and other MCP clients: add to their settings',
                text: config,
                onCopy: () => _copy(config, 'Settings'),
              ),
          ],
        ],
      ),
    );
  }
}

/// A block of text to paste elsewhere, with a copy button.
final class _Snippet extends StatelessWidget {
  const _Snippet(
      {required this.title, required this.text, required this.onCopy});

  final String title;
  final String text;
  final VoidCallback onCopy;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(top: 12),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Row(
            children: <Widget>[
              Expanded(child: Text(title, style: theme.textTheme.labelLarge)),
              IconButton(
                tooltip: 'Copy: $title',
                icon: const Icon(Icons.copy),
                onPressed: onCopy,
              ),
            ],
          ),
          Container(
            width: double.infinity,
            padding: const EdgeInsets.all(8),
            color: theme.colorScheme.surfaceContainerHighest,
            child: SelectableText(text,
                style: const TextStyle(fontFamily: 'monospace')),
          ),
        ],
      ),
    );
  }
}
