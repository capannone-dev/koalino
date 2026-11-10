// Settings → AI connections (#95): the language model servers summaries use (OpenAI-compatible
// or Anthropic, in the cloud or on the local network), the default one, the summary prompt
// template, and automatic summaries of finished meetings (#101). API keys are typed once and
// never shown again.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';

/// Finds the summary prompt field in tests.
const Key summaryTemplateFieldKey = ValueKey<String>('summary-template');

/// Finds the automatic summaries switch in tests.
const Key autoSummarizeSwitchKey = ValueKey<String>('auto-summarize');

/// Lists the AI connections and edits the summary settings.
final class AiSettingsScreen extends StatefulWidget {
  /// Creates the page. Side effects: Loads the settings when first shown.
  const AiSettingsScreen({required this.controller, super.key});

  final SummariesController controller;

  @override
  State<AiSettingsScreen> createState() => _AiSettingsScreenState();
}

/// Holds the template being edited.
final class _AiSettingsScreenState extends State<AiSettingsScreen> {
  final TextEditingController _template = TextEditingController();

  /// The saved template the field was last filled from, to refill it after a save or reset.
  String? _shownTemplate;

  @override
  void initState() {
    super.initState();
    widget.controller.load();
  }

  @override
  void dispose() {
    _template.dispose();
    super.dispose();
  }

  /// Opens the connection form; `existing` edits that connection.
  Future<void> _editConnection(LlmConnection? existing) async {
    final LlmConnectionInput? input = await showDialog<LlmConnectionInput>(
      context: context,
      builder: (BuildContext context) => _ConnectionDialog(existing: existing),
    );
    if (input != null) {
      await widget.controller.saveConnection(input, id: existing?.id);
    }
  }

  /// Asks before removing a connection and its key.
  Future<void> _remove(LlmConnection connection) async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text('Remove ${connection.label}?'),
        content: const Text(
            'Its API key is deleted from this device. Summaries already written stay.'),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Remove'),
          ),
        ],
      ),
    );
    if (confirmed ?? false) {
      await widget.controller.deleteConnection(connection.id);
    }
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Scaffold(
      appBar: AppBar(title: const Text('AI connections')),
      floatingActionButton: FloatingActionButton.extended(
        icon: const Icon(Icons.add),
        label: const Text('Add connection'),
        onPressed: () => _editConnection(null),
      ),
      body: ListenableBuilder(
        listenable: widget.controller,
        builder: (BuildContext context, Widget? child) {
          final SummariesController controller = widget.controller;
          final LlmSettings? settings = controller.settings;
          if (settings != null && settings.template != _shownTemplate) {
            _shownTemplate = settings.template;
            _template.text = settings.template;
          }
          return ListView(
            padding: const EdgeInsets.only(bottom: 96),
            children: <Widget>[
              const Padding(
                padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
                child: Text(
                  'Summaries send the meeting\'s transcript, notes and photos to the '
                  'connection\'s server. A server on your local network keeps them there.',
                ),
              ),
              if (controller.isBusy) const LinearProgressIndicator(),
              if (controller.message case final String message)
                Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                  child: Text(message,
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: theme.colorScheme.primary)),
                ),
              if (settings == null && controller.message == null)
                const Padding(
                  padding: EdgeInsets.all(16),
                  child: Center(child: CircularProgressIndicator()),
                ),
              if (settings != null) ...<Widget>[
                if (settings.connections.isEmpty)
                  const ListTile(
                    leading: Icon(Icons.cloud_off_outlined),
                    title: Text('No connections yet'),
                    subtitle: Text(
                        'Add OpenAI, Anthropic, Mistral, a server on your network, or '
                        'Claude Code or Codex on this computer.'),
                  ),
                for (final LlmConnection connection in settings.connections)
                  _ConnectionTile(
                    connection: connection,
                    busy: controller.isBusy,
                    onMakeDefault: () => controller.makeDefault(connection.id),
                    onTest: () => controller.testConnection(connection.id),
                    onEdit: () => _editConnection(connection),
                    onRemove: () => _remove(connection),
                  ),
                const Divider(height: 32),
                SwitchListTile(
                  key: autoSummarizeSwitchKey,
                  title: const Text('Summarize meetings automatically'),
                  subtitle: const Text(
                      'With the default connection, when a meeting\'s transcript is final.'),
                  value: settings.autoSummarize,
                  onChanged: controller.isBusy
                      ? null
                      : (bool enabled) =>
                          controller.setAutoSummarize(enabled: enabled),
                ),
                const Divider(height: 32),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: TextField(
                    key: summaryTemplateFieldKey,
                    controller: _template,
                    minLines: 6,
                    maxLines: 16,
                    decoration: const InputDecoration(
                      labelText: 'Summary prompt',
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                  child: Text(
                    'Placeholders: ${settings.placeholders.map((String name) => '{{$name}}').join(' ')}. '
                    'The transcript is always sent after the prompt.',
                    style: theme.textTheme.bodySmall,
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.all(8),
                  child: Wrap(
                    spacing: 8,
                    children: <Widget>[
                      FilledButton.tonal(
                        onPressed: controller.isBusy
                            ? null
                            : () => controller.saveTemplate(_template.text),
                        child: const Text('Save prompt'),
                      ),
                      if (settings.customTemplate)
                        TextButton(
                          onPressed: controller.isBusy
                              ? null
                              : controller.resetTemplate,
                          child: const Text('Reset to built-in'),
                        ),
                    ],
                  ),
                ),
              ],
            ],
          );
        },
      ),
    );
  }
}

/// What a connection row's menu does.
enum _ConnectionAction { makeDefault, test, edit, remove }

/// One connection with its badges and actions.
final class _ConnectionTile extends StatelessWidget {
  const _ConnectionTile({
    required this.connection,
    required this.busy,
    required this.onMakeDefault,
    required this.onTest,
    required this.onEdit,
    required this.onRemove,
  });

  final LlmConnection connection;
  final bool busy;
  final VoidCallback onMakeDefault;
  final VoidCallback onTest;
  final VoidCallback onEdit;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    final bool tool = connection.kind.isLocalCli;
    final List<String> notes = <String>[
      '${connection.kind.label} · ${connection.model.isEmpty ? 'default model' : connection.model}',
      if (connection.isDefault) 'Default',
      if (tool)
        connection.toolInstalled ? 'Installed' : 'Not found on this computer'
      else
        connection.hasKey ? 'API key saved' : 'No API key',
      if (connection.plainHttp) 'Unencrypted http, local network only',
    ];
    final String where = tool
        ? 'Runs with your plan\'s sign-in on this computer'
        : connection.baseUrl;
    return ListTile(
      leading:
          Icon(connection.isDefault ? Icons.star : Icons.auto_awesome_outlined),
      title: Text(connection.label),
      subtitle: Text('${notes.join(' · ')}\n$where'),
      isThreeLine: true,
      trailing: PopupMenuButton<_ConnectionAction>(
        tooltip: 'Actions for ${connection.label}',
        enabled: !busy,
        onSelected: (_ConnectionAction action) => switch (action) {
          _ConnectionAction.makeDefault => onMakeDefault(),
          _ConnectionAction.test => onTest(),
          _ConnectionAction.edit => onEdit(),
          _ConnectionAction.remove => onRemove(),
        },
        itemBuilder: (BuildContext context) =>
            <PopupMenuEntry<_ConnectionAction>>[
          if (!connection.isDefault)
            const PopupMenuItem<_ConnectionAction>(
              value: _ConnectionAction.makeDefault,
              child: Text('Make default'),
            ),
          const PopupMenuItem<_ConnectionAction>(
            value: _ConnectionAction.test,
            child: Text('Test'),
          ),
          const PopupMenuItem<_ConnectionAction>(
            value: _ConnectionAction.edit,
            child: Text('Edit'),
          ),
          const PopupMenuItem<_ConnectionAction>(
            value: _ConnectionAction.remove,
            child: Text('Remove'),
          ),
        ],
      ),
    );
  }
}

/// The form for a new or existing connection; it pops a `LlmConnectionInput` when saved.
final class _ConnectionDialog extends StatefulWidget {
  const _ConnectionDialog({this.existing});

  final LlmConnection? existing;

  @override
  State<_ConnectionDialog> createState() => _ConnectionDialogState();
}

/// Holds the fields.
final class _ConnectionDialogState extends State<_ConnectionDialog> {
  late final TextEditingController _label =
      TextEditingController(text: widget.existing?.label ?? '');
  late final TextEditingController _baseUrl = TextEditingController(
      text: widget.existing?.baseUrl ?? llmPresets.first.baseUrl);
  late final TextEditingController _model = TextEditingController(
      text: widget.existing?.model ?? llmPresets.first.model);
  late final TextEditingController _context = TextEditingController(
      text:
          '${widget.existing?.contextTokens ?? llmPresets.first.contextTokens}');
  final TextEditingController _key = TextEditingController();
  late LlmKind _kind = widget.existing?.kind ?? llmPresets.first.kind;
  bool _clearKey = false;
  String? _error;

  @override
  void dispose() {
    for (final TextEditingController field in <TextEditingController>[
      _label,
      _baseUrl,
      _model,
      _context,
      _key,
    ]) {
      field.dispose();
    }
    super.dispose();
  }

  /// Fills the fields from a preset.
  void _applyPreset(LlmPreset preset) => setState(() {
        _kind = preset.kind;
        _baseUrl.text = preset.baseUrl;
        _model.text = preset.model;
        _context.text = '${preset.contextTokens}';
        if (_label.text.trim().isEmpty) {
          _label.text = preset.name;
        }
      });

  /// Checks the fields and pops the input.
  void _save() {
    final int? contextTokens = int.tryParse(_context.text.trim());
    if (_kind.isLocalCli) {
      if (_label.text.trim().isEmpty) {
        setState(() => _error = 'Enter a name.');
        return;
      }
    } else if (_label.text.trim().isEmpty ||
        _baseUrl.text.trim().isEmpty ||
        _model.text.trim().isEmpty) {
      setState(() => _error = 'Enter a name, an address and a model.');
      return;
    }
    if (contextTokens == null || contextTokens <= 0) {
      setState(
          () => _error = 'The context is a number of tokens, such as 8192.');
      return;
    }
    Navigator.of(context).pop(LlmConnectionInput(
      label: _label.text.trim(),
      kind: _kind,
      // Claude Code and Codex have no address and never get a key (#105).
      baseUrl: _kind.isLocalCli ? '' : _baseUrl.text.trim(),
      model: _model.text.trim(),
      contextTokens: contextTokens,
      apiKey: _kind.isLocalCli ? null : _key.text,
      clearKey: !_kind.isLocalCli && _clearKey,
    ));
  }

  @override
  Widget build(BuildContext context) {
    final LlmConnection? existing = widget.existing;
    return AlertDialog(
      title:
          Text(existing == null ? 'Add connection' : 'Edit ${existing.label}'),
      content: SizedBox(
        width: 520,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: <Widget>[
              if (existing == null) ...<Widget>[
                Wrap(
                  spacing: 8,
                  runSpacing: 4,
                  children: <Widget>[
                    for (final LlmPreset preset in availableLlmPresets)
                      ActionChip(
                        label: Text(preset.name),
                        onPressed: () => _applyPreset(preset),
                      ),
                  ],
                ),
                const SizedBox(height: 12),
              ],
              TextField(
                controller: _label,
                decoration: const InputDecoration(labelText: 'Name'),
              ),
              DropdownButtonFormField<LlmKind>(
                // Rebuilt when a preset changes the protocol, since the field keeps its own value.
                key: ValueKey<LlmKind>(_kind),
                initialValue: _kind,
                decoration: const InputDecoration(labelText: 'Protocol'),
                items: <DropdownMenuItem<LlmKind>>[
                  for (final LlmKind kind in LlmKind.values)
                    if (!kind.isLocalCli || canRunLocalCli || kind == _kind)
                      DropdownMenuItem<LlmKind>(
                          value: kind, child: Text(kind.label)),
                ],
                onChanged: (LlmKind? kind) =>
                    setState(() => _kind = kind ?? _kind),
              ),
              if (_kind.isLocalCli)
                const Padding(
                  padding: EdgeInsets.only(top: 12),
                  child: Text(
                      'Runs the tool installed and signed in on this computer, so your '
                      'Claude or ChatGPT plan writes the summaries instead of API credit. '
                      'The meeting goes to Anthropic or OpenAI under that account.'),
                ),
              if (!_kind.isLocalCli)
                TextField(
                  controller: _baseUrl,
                  keyboardType: TextInputType.url,
                  decoration: const InputDecoration(
                    labelText: 'Address',
                    helperText:
                        'https:// always; http:// only on this device or the local network.',
                  ),
                ),
              TextField(
                controller: _model,
                decoration: InputDecoration(
                  labelText: 'Model',
                  helperText: _kind.isLocalCli
                      ? 'Leave empty for the tool\'s own default.'
                      : null,
                ),
              ),
              TextField(
                controller: _context,
                keyboardType: TextInputType.number,
                decoration:
                    const InputDecoration(labelText: 'Context (tokens)'),
              ),
              if (!_kind.isLocalCli)
                TextField(
                  controller: _key,
                  obscureText: true,
                  enableSuggestions: false,
                  autocorrect: false,
                  decoration: InputDecoration(
                    labelText: 'API key',
                    helperText: existing?.hasKey ?? false
                        ? 'A key is saved. Leave empty to keep it.'
                        : 'Leave empty for a local server without keys.',
                  ),
                ),
              if (!_kind.isLocalCli && (existing?.hasKey ?? false))
                CheckboxListTile(
                  contentPadding: EdgeInsets.zero,
                  value: _clearKey,
                  title: const Text('Remove the saved key'),
                  onChanged: (bool? value) =>
                      setState(() => _clearKey = value ?? false),
                ),
              if (_error case final String error)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(error,
                      style: TextStyle(
                          color: Theme.of(context).colorScheme.error)),
                ),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Cancel'),
        ),
        FilledButton(onPressed: _save, child: const Text('Save connection')),
      ],
    );
  }
}
