// The Settings page for the Whisper model and OpenAI (Issue #67). Models are grouped by size. The
// OpenAI key field is write-only, and choosing OpenAI needs an explicit confirmation.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/transcription_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';

/// Finds the OpenAI key field in tests.
const Key openAiKeyFieldKey = ValueKey<String>('openai-key');

/// Finds the provider selector in tests.
const Key providerSelectorKey = ValueKey<String>('transcription-provider');

/// Finds the button applying the suggested model in tests.
const Key useSuggestionKey = ValueKey<String>('use-suggestion');

/// Finds the efficient mode switch in tests.
const Key efficientModeKey = ValueKey<String>('efficient-mode');

/// Speed and accuracy hints for each size, from measurements on Italian speech (see #67, #69).
const Map<String, String> _familyHints = <String, String>{
  'tiny': 'Fastest, least accurate',
  'base': 'Fast, many mistakes. The app ships with this one',
  'small': 'Good balance on most computers',
  'medium': 'Accurate, slower, needs about 2 GB of memory',
  'large-v1': 'Older, needs a powerful computer',
  'large-v2': 'Older, needs a powerful computer',
  'large-v3': 'Most accurate, needs a powerful computer',
  'large-v3-turbo': 'Very accurate. Best with a graphics card',
};

/// Formats a size in plain units.
String formatModelSize(int bytes) => bytes >= 1000000000
    ? '${(bytes / 1000000000).toStringAsFixed(1)} GB'
    : '${(bytes / 1000000).round()} MB';

/// Shows the models and the provider choice.
final class TranscriptionSettingsScreen extends StatefulWidget {
  /// Creates the page.
  ///
  /// Inputs: The controller and the default transcription language's code, for the warning on
  /// English-only models.
  const TranscriptionSettingsScreen({
    required this.controller,
    required this.languageCode,
    super.key,
  });

  final TranscriptionSettingsController controller;
  final String languageCode;

  @override
  State<TranscriptionSettingsScreen> createState() =>
      _TranscriptionSettingsScreenState();
}

/// Holds only the typed key, which is cleared as soon as it is saved.
final class _TranscriptionSettingsScreenState
    extends State<TranscriptionSettingsScreen> {
  final TextEditingController _keyField = TextEditingController();
  bool _replacingKey = false;

  @override
  void initState() {
    super.initState();
    widget.controller.load();
  }

  @override
  void dispose() {
    _keyField.dispose();
    super.dispose();
  }

  /// Confirms that recordings will leave the device, then selects OpenAI.
  Future<void> _chooseOpenAi() async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: const Text('Send recordings to OpenAI?'),
        content: const Text(
            'Your recordings will leave this device and go to OpenAI to be '
            'transcribed, under your OpenAI account. If OpenAI can\'t be reached, this '
            'device transcribes instead, and nothing is uploaded later.'),
        actions: <Widget>[
          TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: const Text('Cancel')),
          FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: const Text('Send to OpenAI')),
        ],
      ),
    );
    if (confirmed ?? false) {
      await widget.controller.setProvider('openai');
    }
  }

  /// Warns before a download that may not run well, then starts it.
  Future<void> _download(WhisperModelInfo model) async {
    if (model.tooLargeForMemory) {
      final bool? confirmed = await showDialog<bool>(
        context: context,
        builder: (BuildContext context) => AlertDialog(
          title: const Text('This model may be too large'),
          content:
              Text('${model.name} may need more memory than this device has, '
                  'and recordings could fail or run slowly.'),
          actions: <Widget>[
            TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: const Text('Cancel')),
            FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: const Text('Download anyway')),
          ],
        ),
      );
      if (!(confirmed ?? false)) {
        return;
      }
    }
    await widget.controller.download(model.fileName);
  }

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Transcription')),
        body: ListenableBuilder(
          listenable: widget.controller,
          builder: (BuildContext context, Widget? child) {
            final TranscriptionSettings? settings = widget.controller.settings;
            if (settings == null) {
              return const Center(
                  child: Text('Transcription settings are unavailable.'));
            }
            return ListView(
              children: <Widget>[
                if (widget.controller.message case final String message)
                  Padding(
                    padding: const EdgeInsets.all(16),
                    child: Text(message),
                  ),
                ..._providerTiles(settings),
                const Divider(),
                ..._modelTiles(context, settings),
              ],
            );
          },
        ),
      );

  /// The provider choice and the key field.
  List<Widget> _providerTiles(TranscriptionSettings settings) {
    final TranscriptionSettingsController controller = widget.controller;
    return <Widget>[
      ListTile(
        leading: const Icon(Icons.cloud_outlined),
        title: const Text('Transcribe with'),
        subtitle: Text(settings.provider == 'openai'
            ? 'OpenAI receives your recordings. Without a connection, this device transcribes.'
            : 'Recordings stay on this device.'),
        trailing: DropdownButton<String>(
          key: providerSelectorKey,
          value: settings.provider,
          underline: const SizedBox.shrink(),
          onChanged: controller.isBusy
              ? null
              : (String? chosen) {
                  if (chosen == 'openai' && settings.provider != 'openai') {
                    _chooseOpenAi();
                  } else if (chosen == 'local') {
                    controller.setProvider('local');
                  }
                },
          items: const <DropdownMenuItem<String>>[
            DropdownMenuItem<String>(
                value: 'local', child: Text('This device')),
            DropdownMenuItem<String>(value: 'openai', child: Text('OpenAI')),
          ],
        ),
      ),
      if (settings.usingLocalFallback)
        const ListTile(
          leading: Icon(Icons.info_outline),
          title:
              Text('OpenAI can\'t be reached, so this device is transcribing.'),
        ),
      Padding(
        padding: const EdgeInsets.all(16),
        child: _keySection(settings),
      ),
    ];
  }

  /// The write-only key field, or that a key is saved with Replace, Remove and Test.
  Widget _keySection(TranscriptionSettings settings) {
    final TranscriptionSettingsController controller = widget.controller;
    if (!settings.keychainAvailable) {
      return const Text(
          'This device has no keychain, so an OpenAI key can\'t be saved.');
    }
    if (settings.keySaved && !_replacingKey) {
      return Wrap(
        spacing: 8,
        runSpacing: 8,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: <Widget>[
          const Text('An OpenAI key is saved.'),
          OutlinedButton(
              onPressed: () => setState(() => _replacingKey = true),
              child: const Text('Replace')),
          OutlinedButton(
              onPressed: controller.isBusy ? null : controller.removeKey,
              child: const Text('Remove')),
          OutlinedButton(
              onPressed: controller.isBusy ? null : controller.testKey,
              child: const Text('Test key')),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        TextField(
          key: openAiKeyFieldKey,
          controller: _keyField,
          obscureText: true,
          autocorrect: false,
          enableSuggestions: false,
          decoration: const InputDecoration(
            labelText: 'OpenAI API key',
            border: OutlineInputBorder(),
          ),
        ),
        const SizedBox(height: 8),
        FilledButton(
          onPressed: controller.isBusy
              ? null
              : () async {
                  final String key = _keyField.text;
                  _keyField.clear();
                  setState(() => _replacingKey = false);
                  await controller.saveKey(key);
                },
          child: const Text('Save key'),
        ),
      ],
    );
  }

  /// The models, grouped by size in catalog order.
  List<Widget> _modelTiles(
      BuildContext context, TranscriptionSettings settings) {
    final List<Widget> tiles = <Widget>[
      ListTile(
        title: Text('Whisper model',
            style: Theme.of(context).textTheme.titleMedium),
        subtitle: const Text(
            'Bigger models make fewer mistakes, and need more time and '
            'memory. Downloads come from Hugging Face. This choice applies '
            'only to this device.'),
      ),
      ..._suggestionTiles(settings),
      ..._efficientTiles(settings),
    ];
    String? family;
    for (final WhisperModelInfo model in settings.models) {
      if (model.family != family) {
        family = model.family;
        tiles.add(Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
          child: Text('${model.family}: ${_familyHints[model.family] ?? ''}',
              style: Theme.of(context).textTheme.labelLarge),
        ));
      }
      tiles.add(_modelTile(settings, model));
    }
    return tiles;
  }

  /// The model suggested for this device, with a button unless it is already in use.
  List<Widget> _suggestionTiles(TranscriptionSettings settings) {
    final WhisperSuggestion? suggestion = settings.suggestion;
    final WhisperModelInfo? model = settings.models
        .where((WhisperModelInfo row) => row.fileName == suggestion?.fileName)
        .firstOrNull;
    if (suggestion == null || model == null) {
      return const <Widget>[];
    }
    final TranscriptionSettingsController controller = widget.controller;
    final bool inUse = settings.selectedModel == model.fileName &&
        settings.efficient.enabled == suggestion.efficient;
    return <Widget>[
      ListTile(
        leading: const Icon(Icons.recommend_outlined),
        title: Text('Suggested for this device: ${model.name}'
            '${suggestion.efficient ? ', efficient mode' : ''}'),
        subtitle: Text(suggestion.reason),
        trailing: inUse
            ? null
            : FilledButton.tonal(
                key: useSuggestionKey,
                onPressed: controller.isBusy || controller.isDownloading
                    ? null
                    : () => controller.applySuggestion(suggestion,
                        onDevice: model.onDevice),
                child: Text(model.onDevice
                    ? 'Use'
                    : 'Download ${formatModelSize(model.sizeBytes)}'),
              ),
      ),
    ];
  }

  /// The efficient mode switch, only with a supported GPU, and any crash that stopped the GPU.
  List<Widget> _efficientTiles(TranscriptionSettings settings) {
    final TranscriptionSettingsController controller = widget.controller;
    final EfficientModeStatus efficient = settings.efficient;
    return <Widget>[
      if (efficient.gpuName case final String gpuName)
        SwitchListTile(
          key: efficientModeKey,
          secondary: const Icon(Icons.bolt_outlined),
          title: const Text('Efficient mode'),
          subtitle:
              Text('Runs the same models on the graphics chip ($gpuName), '
                  'usually faster. Off uses the processor, as standard.'),
          value: efficient.enabled,
          onChanged: controller.isBusy
              ? null
              : (bool enabled) => controller.setEfficientGpu(enabled: enabled),
        ),
      if (efficient.gpuBlocked)
        ListTile(
          leading: const Icon(Icons.warning_amber_outlined),
          title: const Text('The graphics chip stopped the app last time, so '
              'this device is transcribing with its processor.'),
          subtitle: Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: controller.isBusy ? null : controller.retryGpu,
              child: const Text('Try the graphics chip again'),
            ),
          ),
        ),
    ];
  }

  /// One model's row.
  Widget _modelTile(TranscriptionSettings settings, WhisperModelInfo model) {
    final TranscriptionSettingsController controller = widget.controller;
    final WhisperDownload download = settings.download;
    final bool downloadingThis =
        download.state == 'downloading' && download.fileName == model.fileName;
    final bool selected = settings.selectedModel == model.fileName;
    final bool wrongLanguage = model.englishOnly &&
        widget.languageCode != 'en' &&
        widget.languageCode != 'auto';
    final List<String> notes = <String>[
      formatModelSize(model.sizeBytes),
      if (model.onDevice) 'on this device',
      if (model.englishOnly) 'English only',
      if (model.tooLargeForMemory) 'may be too large for this device',
      if (wrongLanguage) 'doesn\'t match your language',
      if ((settings.availableDiskBytes ?? model.sizeBytes) < model.sizeBytes &&
          !model.onDevice)
        'not enough free space',
    ];
    return ListTile(
      title: Text(model.name),
      subtitle: downloadingThis
          ? LinearProgressIndicator(
              value: download.totalBytes == 0
                  ? null
                  : download.receivedBytes / download.totalBytes)
          : Text(notes.join(' · ')),
      leading: Icon(
          selected ? Icons.radio_button_checked : Icons.radio_button_unchecked),
      onTap: model.onDevice && !selected && !controller.isBusy
          ? () => controller.selectModel(model.fileName)
          : null,
      trailing: Wrap(
        children: <Widget>[
          if (downloadingThis)
            IconButton(
              tooltip: 'Cancel download',
              icon: const Icon(Icons.close),
              onPressed: controller.cancelDownload,
            )
          else if (!model.onDevice)
            IconButton(
              tooltip: 'Download ${model.name}',
              icon: const Icon(Icons.download),
              onPressed: controller.isDownloading || controller.isBusy
                  ? null
                  : () => _download(model),
            )
          else if (model.removable)
            IconButton(
              tooltip: 'Remove ${model.name}',
              icon: const Icon(Icons.delete_outline),
              onPressed: controller.isBusy
                  ? null
                  : () => controller.removeModel(model.fileName),
            ),
        ],
      ),
    );
  }
}
