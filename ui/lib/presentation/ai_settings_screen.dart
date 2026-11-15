// Settings → AI connections (#95): the language model servers summaries use (an OpenAI-compatible
// server on this device or the local network, or Claude Code and Codex signed in on this
// computer), the default one, the summary prompt template, and automatic summaries of finished
// meetings (#101), and on a computer the connection that writes the summaries paired phones ask
// for (#107). The app keeps no API keys (#117).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Finds the summary prompt field in tests.
const Key summaryTemplateFieldKey = ValueKey<String>('summary-template');

/// Finds the automatic summaries switch in tests.
const Key autoSummarizeSwitchKey = ValueKey<String>('auto-summarize');

/// Finds the menu of the connection that answers paired devices in tests (#107).
const Key pairedSummaryConnectionKey =
    ValueKey<String>('paired-summary-connection');

/// The menu value that stops answering paired devices.
const String _notAnswering = '';

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
        title: Text(l10n.remove(connection.label)),
        content: Text(l10n.itsApiKeyIsDeletedFrom),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l10n.remove2),
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
      appBar: AppBar(title: Text(l10n.aiConnections)),
      floatingActionButton: FloatingActionButton.extended(
        icon: const Icon(Icons.add),
        label: Text(l10n.addConnection),
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
              Padding(
                padding: EdgeInsets.fromLTRB(16, 16, 16, 8),
                child: Text(
                  l10n.summariesSendTheMeetingSTranscript,
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
                  ListTile(
                    leading: Icon(Icons.cloud_off_outlined),
                    title: Text(l10n.noConnectionsYet),
                    subtitle: Text(l10n.addAServerOnYourNetwork),
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
                  title: Text(l10n.summarizeMeetingsAutomatically),
                  subtitle: Text(l10n.withTheDefaultConnectionWhenA),
                  value: settings.autoSummarize,
                  onChanged: controller.isBusy
                      ? null
                      : (bool enabled) =>
                          controller.setAutoSummarize(enabled: enabled),
                ),
                if (!controller.asksPairedComputer &&
                    settings.connections.isNotEmpty)
                  ListTile(
                    title: Text(l10n.summariesForMyOtherDevices),
                    subtitle: Text(l10n.aPairedPhoneCanAskThis),
                    trailing: DropdownButton<String>(
                      key: pairedSummaryConnectionKey,
                      value: settings.pairedSummaryConnection ?? _notAnswering,
                      onChanged: controller.isBusy
                          ? null
                          : (String? id) =>
                              controller.setPairedSummaryConnection(
                                  id == null || id == _notAnswering
                                      ? null
                                      : id),
                      items: <DropdownMenuItem<String>>[
                        DropdownMenuItem<String>(
                          value: _notAnswering,
                          child: Text(l10n.off),
                        ),
                        for (final LlmConnection connection
                            in settings.connections)
                          DropdownMenuItem<String>(
                            value: connection.id,
                            child: Text(connection.label),
                          ),
                      ],
                    ),
                  ),
                const Divider(height: 32),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 16),
                  child: TextField(
                    key: summaryTemplateFieldKey,
                    controller: _template,
                    minLines: 6,
                    maxLines: 16,
                    decoration: InputDecoration(
                      labelText: l10n.summaryPrompt,
                      border: OutlineInputBorder(),
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 8, 16, 0),
                  child: Text(
                    l10n.placeholdersTheTranscriptIsAlwaysSent(settings
                        .placeholders
                        .map((String name) => '{{$name}}')
                        .join(' ')),
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
                        child: Text(l10n.savePrompt),
                      ),
                      if (settings.customTemplate)
                        TextButton(
                          onPressed: controller.isBusy
                              ? null
                              : controller.resetTemplate,
                          child: Text(l10n.resetToBuiltIn),
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
      if (connection.isDefault) l10n.defaultLabel,
      if (tool)
        connection.toolInstalled ? l10n.installed : l10n.notFoundOnThisComputer,
      if (connection.plainHttp) l10n.unencryptedHttpLocalNetworkOnly,
    ];
    final String where = tool ? l10n.runsWithYourPlanSSign : connection.baseUrl;
    return ListTile(
      leading:
          Icon(connection.isDefault ? Icons.star : Icons.auto_awesome_outlined),
      title: Text(connection.label),
      subtitle: Text('${notes.join(' · ')}\n$where'),
      isThreeLine: true,
      trailing: PopupMenuButton<_ConnectionAction>(
        tooltip: l10n.actionsFor(connection.label),
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
            PopupMenuItem<_ConnectionAction>(
              value: _ConnectionAction.makeDefault,
              child: Text(l10n.makeDefault),
            ),
          PopupMenuItem<_ConnectionAction>(
            value: _ConnectionAction.test,
            child: Text(l10n.test),
          ),
          PopupMenuItem<_ConnectionAction>(
            value: _ConnectionAction.edit,
            child: Text(l10n.edit),
          ),
          PopupMenuItem<_ConnectionAction>(
            value: _ConnectionAction.remove,
            child: Text(l10n.remove2),
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
  late LlmKind _kind = widget.existing?.kind ?? llmPresets.first.kind;
  String? _error;

  @override
  void dispose() {
    for (final TextEditingController field in <TextEditingController>[
      _label,
      _baseUrl,
      _model,
      _context,
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
        setState(() => _error = l10n.enterAName);
        return;
      }
    } else if (_label.text.trim().isEmpty ||
        _baseUrl.text.trim().isEmpty ||
        _model.text.trim().isEmpty) {
      setState(() => _error = l10n.enterANameAnAddressAnd);
      return;
    }
    if (contextTokens == null || contextTokens <= 0) {
      setState(() => _error = l10n.theContextIsANumberOf);
      return;
    }
    Navigator.of(context).pop(LlmConnectionInput(
      label: _label.text.trim(),
      kind: _kind,
      // Claude Code and Codex have no address (#105).
      baseUrl: _kind.isLocalCli ? '' : _baseUrl.text.trim(),
      model: _model.text.trim(),
      contextTokens: contextTokens,
    ));
  }

  @override
  Widget build(BuildContext context) {
    final LlmConnection? existing = widget.existing;
    return AlertDialog(
      title: Text(
          existing == null ? l10n.addConnection : l10n.edit2(existing.label)),
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
                decoration: InputDecoration(labelText: l10n.name),
              ),
              DropdownButtonFormField<LlmKind>(
                // Rebuilt when a preset changes the protocol, since the field keeps its own value.
                key: ValueKey<LlmKind>(_kind),
                initialValue: _kind,
                decoration: InputDecoration(labelText: l10n.protocol),
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
                Padding(
                  padding: EdgeInsets.only(top: 12),
                  child: Text(l10n.runsTheToolInstalledAndSigned),
                ),
              if (!_kind.isLocalCli)
                TextField(
                  controller: _baseUrl,
                  keyboardType: TextInputType.url,
                  decoration: InputDecoration(
                    labelText: l10n.address,
                    helperText: l10n.httpsAlwaysHttpOnlyOnThis,
                  ),
                ),
              TextField(
                controller: _model,
                decoration: InputDecoration(
                  labelText: l10n.model,
                  helperText:
                      _kind.isLocalCli ? l10n.leaveEmptyForTheToolS : null,
                ),
              ),
              TextField(
                controller: _context,
                keyboardType: TextInputType.number,
                decoration: InputDecoration(labelText: l10n.contextTokens),
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
          child: Text(l10n.cancel),
        ),
        FilledButton(onPressed: _save, child: Text(l10n.saveConnection)),
      ],
    );
  }
}
