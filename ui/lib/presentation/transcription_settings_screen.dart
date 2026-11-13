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

/// Finds the button applying the proposed model in tests.
const Key useProposalKey = ValueKey<String>('use-proposal');

/// Finds the button dismissing the proposed model in tests.
const Key dismissProposalKey = ValueKey<String>('dismiss-proposal');

/// Finds the button applying the suggested model in tests.
const Key useSuggestionKey = ValueKey<String>('use-suggestion');

/// Finds the efficient mode switch in tests.
const Key efficientModeKey = ValueKey<String>('efficient-mode');

/// Finds the glossary's text field.
const Key glossaryFieldKey = ValueKey<String>('glossary-field');

/// Finds the speech retention window's menu (#93).
const Key keepSpeechKey = ValueKey<String>('keep-speech');

/// Finds the "Automatic" model row (#106).
const Key automaticModelKey = ValueKey<String>('automatic-model');

/// Finds the button downloading the model Automatic waits for.
const Key automaticDownloadKey = ValueKey<String>('automatic-download');

/// Finds the after-meeting pass switch (#87).
const Key afterMeetingPassKey = ValueKey<String>('after-meeting-pass');

/// The after-meeting pass's model picker (#87).
const Key afterMeetingPassModelKey =
    ValueKey<String>('after-meeting-pass-model');

/// Finds the switch that records the computer's sound (#108).
const Key computerSoundKey = ValueKey<String>('computer-sound');

/// Finds the button that saves the glossary.
const Key glossarySaveKey = ValueKey<String>('glossary-save');

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
                const Divider(),
                // Keyed, so the message row above appearing keeps its text and state.
                _GlossarySection(
                    key: const ValueKey<String>('glossary-section'),
                    controller: widget.controller,
                    saved: settings.glossary),
                const Divider(),
                if (settings.computerSound.available)
                  _computerSoundTile(settings.computerSound.enabled),
                _afterMeetingPassTile(settings.afterMeetingPass),
                if (settings.afterMeetingPass)
                  _afterMeetingPassModelTile(settings),
                _keepSpeechTile(settings.keepSpeech),
              ],
            );
          },
        ),
      );

  /// Recording what the computer plays (#108): the other side of a call, as its own track.
  Widget _computerSoundTile(bool enabled) => SwitchListTile(
        key: computerSoundKey,
        secondary: const Icon(Icons.speaker_outlined),
        title: const Text("Also record the computer's sound"),
        subtitle: const Text('Transcribes what the computer plays, such as '
            'the other side of a call, beside the microphone. When the '
            'microphone also hears the speakers, the repeated words are '
            'kept once.'),
        value: enabled,
        onChanged: widget.controller.isBusy
            ? null
            : (bool value) =>
                widget.controller.setComputerSound(enabled: value),
      );

  /// The after-meeting pass (#87): transcribe the meeting again after Stop, with longer context.
  /// The speech is kept encrypted until then and deleted afterwards.
  Widget _afterMeetingPassTile(bool enabled) => SwitchListTile(
        key: afterMeetingPassKey,
        secondary: const Icon(Icons.auto_fix_high_outlined),
        title: const Text('Improve the transcript after the meeting'),
        subtitle: const Text('Transcribes each meeting again with more '
            'context once it ends; a phone waits until it is charging. The '
            'speech is kept encrypted until then, then deleted.'),
        value: enabled,
        onChanged: widget.controller.isBusy
            ? null
            : (bool value) =>
                widget.controller.setAfterMeetingPass(enabled: value),
      );

  /// The model the after-meeting pass re-transcribes with (#87): the recording model, or one
  /// already on this device (a larger one gains most, as it has time after the meeting).
  Widget _afterMeetingPassModelTile(TranscriptionSettings settings) {
    final List<WhisperModelInfo> onDevice = settings.models
        .where((WhisperModelInfo model) => model.onDevice)
        .toList(growable: false);
    final String? chosen = onDevice.any((WhisperModelInfo model) =>
            model.fileName == settings.afterMeetingPassModel)
        ? settings.afterMeetingPassModel
        : null;
    return ListTile(
      leading: const Icon(Icons.model_training_outlined),
      title: const Text('Model for this pass'),
      subtitle: const Text('A larger model is slower but more accurate, and '
          'the pass has time after the meeting. Download models above.'),
      trailing: DropdownButton<String?>(
        key: afterMeetingPassModelKey,
        value: chosen,
        onChanged: widget.controller.isBusy
            ? null
            : (String? fileName) =>
                widget.controller.setAfterMeetingPassModel(fileName),
        items: <DropdownMenuItem<String?>>[
          const DropdownMenuItem<String?>(
              value: null, child: Text('Same as for recording')),
          for (final WhisperModelInfo model in onDevice)
            DropdownMenuItem<String?>(
                value: model.fileName, child: Text(model.name)),
        ],
      ),
    );
  }

  /// The speech retention window: how long a recording's speech waits for another device's
  /// recording of the same meeting before it is deleted (#93).
  Widget _keepSpeechTile(SpeechRetention retention) => ListTile(
        leading: const Icon(Icons.timer_outlined),
        title: const Text('Keep speech for combining'),
        subtitle: const Text('When a meeting was also recorded on another '
            'device, its speech waits this long to be combined, then is '
            'deleted.'),
        trailing: DropdownButton<int>(
          key: keepSpeechKey,
          value: retention.days,
          onChanged: widget.controller.isBusy
              ? null
              : (int? days) {
                  if (days != null && days != retention.days) {
                    widget.controller.setKeepSpeechDays(days);
                  }
                },
          items: <DropdownMenuItem<int>>[
            for (final int days in retention.choices)
              DropdownMenuItem<int>(
                  value: days, child: Text(days == 1 ? '1 day' : '$days days')),
          ],
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
      ..._automaticTiles(settings),
      // A proposal rests on this device's own speed, so it replaces the device-based estimate.
      if (settings.proposal != null)
        ..._proposalTiles(settings)
      else if (!settings.automatic.enabled)
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

  /// The "Automatic" row (#106): the model it runs here and why, and a Download button while the
  /// suggested model isn't on the device. Tapping it chooses Automatic.
  List<Widget> _automaticTiles(TranscriptionSettings settings) {
    final TranscriptionSettingsController controller = widget.controller;
    final AutomaticModelStatus automatic = settings.automatic;
    String nameOf(String? fileName) =>
        settings.models
            .where((WhisperModelInfo row) => row.fileName == fileName)
            .firstOrNull
            ?.name ??
        fileName ??
        'none';
    final WhisperModelInfo? waiting = settings.models
        .where((WhisperModelInfo row) =>
            row.fileName == automatic.waitingForDownload)
        .firstOrNull;
    final String title = automatic.enabled
        ? 'Automatic: ${nameOf(automatic.fileName)}'
            '${automatic.efficient ? ', efficient mode' : ''}'
        : 'Automatic';
    // The suggestion's reason, then how its model ran on this device once timed (#109).
    final String reason = <String>[
      if (settings.suggestion?.reason case final String reason
          when reason.isNotEmpty)
        reason,
      if (automatic.speedCheck case final String speedCheck) speedCheck,
    ].join(' ');
    final String subtitle = waiting != null
        ? 'Uses ${nameOf(automatic.fileName)} until ${waiting.name} is '
            'downloaded, the best fit for this device. $reason'
        : automatic.enabled
            ? reason
            : 'Picks the best model for this device: '
                '${nameOf(automatic.fileName)}'
                '${automatic.efficient ? ', efficient mode' : ''}.';
    final WhisperDownload download = settings.download;
    final bool downloadingWaiting = waiting != null &&
        download.state == 'downloading' &&
        download.fileName == waiting.fileName;
    return <Widget>[
      ListTile(
        key: automaticModelKey,
        leading: Icon(automatic.enabled
            ? Icons.radio_button_checked
            : Icons.radio_button_unchecked),
        title: Text(title),
        subtitle: downloadingWaiting
            ? LinearProgressIndicator(
                value: download.totalBytes == 0
                    ? null
                    : download.receivedBytes / download.totalBytes)
            : Text(subtitle.trim()),
        onTap: automatic.enabled || controller.isBusy
            ? null
            : controller.useAutomaticModel,
        trailing: waiting == null || downloadingWaiting
            ? null
            : FilledButton.tonal(
                key: automaticDownloadKey,
                onPressed: controller.isBusy || controller.isDownloading
                    ? null
                    : () => _download(waiting),
                child: Text('Download ${formatModelSize(waiting.sizeBytes)}'),
              ),
      ),
    ];
  }

  /// A model proposed from this device's measurements, with buttons to accept or dismiss it.
  List<Widget> _proposalTiles(TranscriptionSettings settings) {
    final WhisperProposal? proposal = settings.proposal;
    final WhisperModelInfo? model = settings.models
        .where((WhisperModelInfo row) => row.fileName == proposal?.fileName)
        .firstOrNull;
    if (proposal == null || model == null) {
      return const <Widget>[];
    }
    final TranscriptionSettingsController controller = widget.controller;
    final bool blocked = controller.isBusy || controller.isDownloading;
    return <Widget>[
      ListTile(
        leading: const Icon(Icons.insights_outlined),
        title: Text('Try ${model.name}'),
        subtitle: Text(proposal.reason),
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(16, 0, 16, 8),
        child: Wrap(
          spacing: 8,
          children: <Widget>[
            FilledButton.tonal(
              key: useProposalKey,
              onPressed: blocked
                  ? null
                  : () => controller.applyProposal(proposal,
                      onDevice: model.onDevice),
              child: Text(model.onDevice
                  ? 'Use'
                  : 'Download ${formatModelSize(model.sizeBytes)}'),
            ),
            TextButton(
              key: dismissProposalKey,
              onPressed:
                  blocked ? null : () => controller.dismissProposal(proposal),
              child: const Text('Not now'),
            ),
          ],
        ),
      ),
    ];
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
      // Automatic decides efficient mode itself; a hand-picked model has the switch.
      if (efficient.gpuName case final String gpuName
          when !settings.automatic.enabled)
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
      if (settings.automatic.enabled &&
          settings.automatic.fileName == model.fileName)
        'used by Automatic',
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

/// The glossary: names and terms, one per line, that transcription spells as written.
class _GlossarySection extends StatefulWidget {
  const _GlossarySection(
      {super.key, required this.controller, required this.saved});

  /// Saves the terms.
  final TranscriptionSettingsController controller;

  /// The terms the core holds now.
  final List<String> saved;

  @override
  State<_GlossarySection> createState() => _GlossarySectionState();
}

class _GlossarySectionState extends State<_GlossarySection> {
  late final TextEditingController _terms =
      TextEditingController(text: widget.saved.join('\n'));

  /// Whether the page's last message answers this section's Save, so it is shown here too: the
  /// page shows messages at its top, out of sight below the model list.
  bool _savedHere = false;

  @override
  void didUpdateWidget(_GlossarySection oldWidget) {
    super.didUpdateWidget(oldWidget);
    // Shows what was saved; text the core refused stays in the field to correct.
    if (!_sameTerms(oldWidget.saved, widget.saved)) {
      _terms.text = widget.saved.join('\n');
    }
  }

  @override
  void dispose() {
    _terms.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text('Glossary', style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            const Text('Names, project names and terms, one per line. '
                'Transcription spells them as written here.'),
            const SizedBox(height: 8),
            TextField(
              key: glossaryFieldKey,
              controller: _terms,
              minLines: 3,
              maxLines: 8,
              keyboardType: TextInputType.multiline,
              decoration: const InputDecoration(
                border: OutlineInputBorder(),
                hintText: 'Marco Rossi\nKubernetes\nOKR',
              ),
            ),
            const SizedBox(height: 8),
            Align(
              alignment: Alignment.centerRight,
              child: FilledButton(
                key: glossarySaveKey,
                onPressed: widget.controller.isBusy
                    ? null
                    : () {
                        setState(() => _savedHere = true);
                        widget.controller.setGlossary(_terms.text);
                      },
                child: const Text('Save glossary'),
              ),
            ),
            if (_savedHere)
              if (widget.controller.message case final String message)
                Padding(
                  padding: const EdgeInsets.only(top: 8),
                  child: Text(message),
                ),
          ],
        ),
      );
}

/// Whether two term lists are equal, in order.
bool _sameTerms(List<String> left, List<String> right) =>
    left.length == right.length &&
    Iterable<int>.generate(left.length)
        .every((int index) => left[index] == right[index]);
