// The Settings page for the Whisper model (Issue #67). Models are grouped by size. Recordings are
// always transcribed on the owner's devices: the app keeps no API keys (#117).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/transcription_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/background_core.dart';
import 'package:note_taker_ui/infrastructure/npu_download.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';
import 'package:note_taker_ui/presentation/background_pass_tile.dart';

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

/// The switch for downloading Automatic's model by itself (#189).
const Key autoDownloadKey = ValueKey<String>('auto-download');

/// The after-meeting pass's model picker (#87).
const Key afterMeetingPassModelKey =
    ValueKey<String>('after-meeting-pass-model');

/// Finds the switch that records the computer's sound (#108).
const Key computerSoundKey = ValueKey<String>('computer-sound');

/// Finds the button downloading the NPU files (#75).
const Key npuDownloadKey = ValueKey<String>('npu-download');

/// Finds the switch turning Whisper on the NPU on and off (#75).
const Key npuSwitchKey = ValueKey<String>('npu-switch');

/// Finds the button removing the NPU files (#75).
const Key npuRemoveKey = ValueKey<String>('npu-remove');

/// Finds the button that saves the glossary.
const Key glossarySaveKey = ValueKey<String>('glossary-save');

/// Whether this build ships Whisper `base` inside the app. The F-Droid build (#163) does not: it
/// is built with `--dart-define=KOALINO_NO_BUNDLED_MODELS=true` and downloads models on request.
const bool appBundlesWhisperModel =
    !bool.fromEnvironment('KOALINO_NO_BUNDLED_MODELS');

/// Speed and accuracy hints for each size, from measurements on Italian speech (see #67, #69).
///
/// Inputs: Whether the app ships `base`, which its hint then says. Outputs: Hints by size.
/// Side effects: None.
Map<String, String> _familyHints(bool bundlesWhisperModel) => <String, String>{
      'tiny': l10n.fastestLeastAccurate,
      'base': bundlesWhisperModel
          ? l10n.fastManyMistakesTheAppShips
          : l10n.fastManyMistakes,
      'small': l10n.goodBalanceOnMostComputers,
      'medium': l10n.accurateSlowerNeedsAbout2Gb,
      'large-v1': l10n.olderNeedsAPowerfulComputer,
      'large-v2': l10n.olderNeedsAPowerfulComputer,
      'large-v3': l10n.mostAccurateNeedsAPowerfulComputer,
      'large-v3-turbo': l10n.veryAccurateBestWithAGraphics,
    };

/// The "More models" section's header, which opens the variants (#75).
const Key moreModelsKey = ValueKey<String>('more-models');

/// The sizes listed at once, by their standard multilingual model (#75). The other large models
/// are older or need far more memory than `large-v3-turbo` for little gain.
const Set<String> _mainFamilies = <String>{
  'tiny',
  'base',
  'small',
  'medium',
  'large-v3-turbo',
};

/// The catalog split for display: rows shown at once, and rows under "More models".
typedef ModelRows = ({
  List<WhisperModelInfo> main,
  List<WhisperModelInfo> more
});

/// Splits the catalog so the list shows one row per size, not every variant (#75).
///
/// Inputs: The catalog in its order, and `pinned` file names that must stay visible (the chosen
/// model, Automatic's, a suggestion or proposal).
/// Outputs: `main` holds each main size's standard multilingual model unless it may not fit in
/// memory, plus every model on the device or pinned; `more` holds the rest (quantized,
/// English-only, older or too large), both in catalog order.
/// Side effects: None.
ModelRows splitModelRows(List<WhisperModelInfo> models,
    {required Set<String> pinned}) {
  final List<WhisperModelInfo> main = <WhisperModelInfo>[];
  final List<WhisperModelInfo> more = <WhisperModelInfo>[];
  for (final WhisperModelInfo model in models) {
    final bool standard = _mainFamilies.contains(model.family) &&
        model.name == model.family &&
        !model.tooLargeForMemory;
    if (standard || model.onDevice || pinned.contains(model.fileName)) {
      main.add(model);
    } else {
      more.add(model);
    }
  }
  return (main: main, more: more);
}

/// Formats a size in plain units.
String formatModelSize(int bytes) => bytes >= 1000000000
    ? '${(bytes / 1000000000).toStringAsFixed(1)} GB'
    : '${(bytes / 1000000).round()} MB';

/// Shows the models and the transcription choices.
final class TranscriptionSettingsScreen extends StatefulWidget {
  /// Creates the page.
  ///
  /// Inputs: The controller, the default transcription language's code (for the warning on
  /// English-only models), on a phone the bridge for running the after-meeting pass while the app
  /// is closed (#87), and whether the app ships Whisper `base` (default: this build's setting).
  const TranscriptionSettingsScreen({
    required this.controller,
    required this.languageCode,
    this.backgroundPass,
    this.bundlesWhisperModel = appBundlesWhisperModel,
    super.key,
  });

  final TranscriptionSettingsController controller;
  final String languageCode;

  /// Whether the app ships Whisper `base`; tests set it, the build decides it otherwise.
  final bool bundlesWhisperModel;

  /// Offers the battery exemption the background pass needs; null off Android.
  final BackgroundPassBridge? backgroundPass;

  @override
  State<TranscriptionSettingsScreen> createState() =>
      _TranscriptionSettingsScreenState();
}

/// Loads the settings when shown.
final class _TranscriptionSettingsScreenState
    extends State<TranscriptionSettingsScreen> {
  @override
  void initState() {
    super.initState();
    widget.controller.load();
  }

  /// Warns before a download that may not run well, then starts it.
  Future<void> _download(WhisperModelInfo model) async {
    if (model.tooLargeForMemory) {
      final bool? confirmed = await showDialog<bool>(
        context: context,
        builder: (BuildContext context) => AlertDialog(
          title: Text(l10n.thisModelMayBeTooLarge),
          content: Text(l10n.mayNeedMoreMemoryThanThis(model.name)),
          actions: <Widget>[
            TextButton(
                onPressed: () => Navigator.of(context).pop(false),
                child: Text(l10n.cancel)),
            FilledButton(
                onPressed: () => Navigator.of(context).pop(true),
                child: Text(l10n.downloadAnyway)),
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
        appBar: AppBar(title: Text(l10n.transcription)),
        body: ListenableBuilder(
          listenable: widget.controller,
          builder: (BuildContext context, Widget? child) {
            final TranscriptionSettings? settings = widget.controller.settings;
            if (settings == null) {
              return Center(
                  child: Text(l10n.transcriptionSettingsAreUnavailable));
            }
            return ListView(
              children: <Widget>[
                if (widget.controller.message case final String message)
                  Padding(
                    padding: const EdgeInsets.all(16),
                    child: Text(message),
                  ),
                // On a phone with an AI chip, the NPU is the main choice (#75).
                if (settings.npu.supported) ...<Widget>[
                  ..._npuTiles(settings.npu),
                  const Divider(),
                ],
                ..._modelTiles(context, settings),
                if (settings.automatic.enabled)
                  _autoDownloadTile(settings.autoDownload),
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
                if (settings.afterMeetingPass)
                  if (widget.backgroundPass
                      case final BackgroundPassBridge backgroundPass)
                    BackgroundPassTile(bridge: backgroundPass),
                _keepSpeechTile(settings.keepSpeech),
              ],
            );
          },
        ),
      );

  /// Asks the owner to accept Qualcomm's terms before the NPU files download (#75).
  Future<void> _downloadNpu() async {
    final bool? accepted = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.downloadFromQualcomm),
        content: SingleChildScrollView(
          child: Text(l10n.about360MbIsDownloadedFrom),
        ),
        actions: <Widget>[
          TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.cancel)),
          FilledButton(
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.acceptAndDownload)),
        ],
      ),
    );
    if (accepted ?? false) {
      await widget.controller.downloadNpu();
    }
  }

  /// Whisper on the NPU (#75): download, progress, the switch, and removal.
  List<Widget> _npuTiles(NpuStatus npu) {
    final TranscriptionSettingsController controller = widget.controller;
    const Widget icon = Icon(Icons.memory_outlined);
    if (controller.npuProgress case final NpuDownloadProgress progress) {
      final double? fraction = progress.fraction;
      return <Widget>[
        ListTile(
          leading: icon,
          title: Text(l10n.downloadingTheNpuFiles),
          subtitle: Padding(
            padding: const EdgeInsets.only(top: 8),
            child: LinearProgressIndicator(value: fraction),
          ),
          trailing: TextButton(
            onPressed: controller.cancelNpuDownload,
            child: Text(l10n.cancel),
          ),
        ),
      ];
    }
    if (!npu.installed) {
      return <Widget>[
        ListTile(
          leading: icon,
          title: Text(l10n.transcribeOnTheNpu),
          subtitle: Text(l10n.thisPhoneHasAnAiChip),
          trailing: FilledButton.tonal(
            key: npuDownloadKey,
            onPressed: controller.isBusy ? null : _downloadNpu,
            child: Text(l10n.download),
          ),
        ),
      ];
    }
    return <Widget>[
      SwitchListTile(
        key: npuSwitchKey,
        secondary: icon,
        title: Text(l10n.transcribeOnTheNpu),
        subtitle: Text(switch ((npu.enabled, npu.lastOpen)) {
          (true, 'loaded') =>
            '${l10n.runsWhisperSmallOnThePhone} ${l10n.npuUsedAtTheLastRecording}',
          (true, 'failed') =>
            '${l10n.runsWhisperSmallOnThePhone} ${l10n.npuFailedAtTheLastRecording}',
          _ => l10n.runsWhisperSmallOnThePhone,
        }),
        value: npu.enabled,
        onChanged: controller.isBusy
            ? null
            : (bool enabled) => controller.setNpuWhisper(enabled: enabled),
      ),
      Align(
        alignment: Alignment.centerLeft,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16),
          child: TextButton(
            key: npuRemoveKey,
            onPressed: controller.isBusy ? null : controller.removeNpu,
            child: Text(l10n.removeTheNpuFiles),
          ),
        ),
      ),
    ];
  }

  /// Recording what the computer plays (#108): the other side of a call, as its own track.
  Widget _computerSoundTile(bool enabled) => SwitchListTile(
        key: computerSoundKey,
        secondary: const Icon(Icons.speaker_outlined),
        title: Text(l10n.alsoRecordTheComputerSSound),
        subtitle: Text(l10n.transcribesWhatTheComputerPlaysSuch),
        value: enabled,
        onChanged: widget.controller.isBusy
            ? null
            : (bool value) =>
                widget.controller.setComputerSound(enabled: value),
      );

  /// Downloading Automatic's model by itself (#189): at start, on Wi-Fi when it fits.
  Widget _autoDownloadTile(bool enabled) => SwitchListTile(
        key: autoDownloadKey,
        secondary: const Icon(Icons.download_for_offline_outlined),
        title: Text(l10n.downloadTheBestModelByItself),
        subtitle: Text(l10n.downloadTheBestModelByItselfHint),
        value: enabled,
        onChanged: widget.controller.isBusy
            ? null
            : (bool value) => widget.controller.setAutoDownload(enabled: value),
      );

  /// The after-meeting pass (#87): transcribe the meeting again after Stop, with longer context.
  /// The speech is kept encrypted until then and deleted afterwards.
  Widget _afterMeetingPassTile(bool enabled) => SwitchListTile(
        key: afterMeetingPassKey,
        secondary: const Icon(Icons.auto_fix_high_outlined),
        title: Text(l10n.improveTheTranscriptAfterTheMeeting),
        subtitle: Text(l10n.transcribesEachMeetingAgainWithMore),
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
      title: Text(l10n.modelForThisPass),
      subtitle: Text(l10n.aLargerModelIsSlowerBut),
      trailing: DropdownButton<String?>(
        key: afterMeetingPassModelKey,
        value: chosen,
        onChanged: widget.controller.isBusy
            ? null
            : (String? fileName) =>
                widget.controller.setAfterMeetingPassModel(fileName),
        items: <DropdownMenuItem<String?>>[
          DropdownMenuItem<String?>(
              value: null, child: Text(l10n.sameAsForRecording)),
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
        title: Text(l10n.keepSpeechForCombining),
        subtitle: Text(l10n.whenAMeetingWasAlsoRecorded),
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
                  value: days, child: Text(l10n.dayCount(days))),
          ],
        ),
      );

  /// The models: one row per size, with the variants under "More models" (#75).
  List<Widget> _modelTiles(
      BuildContext context, TranscriptionSettings settings) {
    final List<Widget> tiles = <Widget>[
      ListTile(
        title: Text(l10n.whisperModel,
            style: Theme.of(context).textTheme.titleMedium),
        subtitle: Text(l10n.biggerModelsMakeFewerMistakesAnd),
      ),
      ..._automaticTiles(settings),
      // A proposal rests on this device's own speed, so it replaces the device-based estimate.
      if (settings.proposal != null)
        ..._proposalTiles(settings)
      else if (!settings.automatic.enabled)
        ..._suggestionTiles(settings),
      ..._efficientTiles(settings),
    ];
    final ModelRows rows = splitModelRows(settings.models, pinned: <String>{
      if (settings.selectedModel case final String selected) selected,
      if (settings.automatic.fileName case final String automatic) automatic,
      if (settings.automatic.waitingForDownload case final String waiting)
        waiting,
      if (settings.suggestion?.fileName case final String suggested) suggested,
      if (settings.proposal?.fileName case final String proposed) proposed,
    });
    for (final WhisperModelInfo model in rows.main) {
      tiles.add(_modelTile(settings, model, withHint: true));
    }
    if (rows.more.isNotEmpty) {
      tiles.add(ExpansionTile(
        key: moreModelsKey,
        title: Text(l10n.moreModels(rows.more.length)),
        subtitle: Text(l10n.moreModelsHint),
        children: _groupedModelTiles(context, settings, rows.more),
      ));
    }
    return tiles;
  }

  /// The rows under "More models", with a header per size.
  List<Widget> _groupedModelTiles(BuildContext context,
      TranscriptionSettings settings, List<WhisperModelInfo> models) {
    final List<Widget> tiles = <Widget>[];
    String? family;
    for (final WhisperModelInfo model in models) {
      if (model.family != family) {
        family = model.family;
        tiles.add(Padding(
          padding: const EdgeInsets.fromLTRB(16, 12, 16, 0),
          child: Align(
            alignment: AlignmentDirectional.centerStart,
            child: Text(
                '${model.family}: ${_familyHints(widget.bundlesWhisperModel)[model.family] ?? ''}',
                style: Theme.of(context).textTheme.labelLarge),
          ),
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
        ? l10n.automatic2(nameOf(automatic.fileName),
            automatic.efficient ? ', efficient mode' : '')
        : l10n.automatic;
    // The suggestion's reason, then how its model ran on this device once timed (#109).
    final String reason = <String>[
      if (settings.suggestion?.reason case final String reason
          when reason.isNotEmpty)
        reason,
      if (automatic.speedCheck case final String speedCheck) speedCheck,
    ].join(' ');
    // Said ahead, as on the model rows, so the Download button's refusal is no surprise (#181).
    final bool waitingDoesNotFit = waiting != null &&
        (settings.availableDiskBytes ?? waiting.sizeBytes) < waiting.sizeBytes;
    final String subtitle = waiting != null
        ? <String>[
            l10n
                .usesUntilIsDownloadedTheBest(
                    nameOf(automatic.fileName), waiting.name, reason)
                .trim(),
            if (waitingDoesNotFit) l10n.notEnoughFreeSpaceToDownload,
          ].join(' ')
        : automatic.enabled
            ? reason
            : l10n.picksTheBestModelForThis(nameOf(automatic.fileName),
                automatic.efficient ? ', efficient mode' : '');
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
                child: Text(l10n.download2(formatModelSize(waiting.sizeBytes))),
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
        title: Text(l10n.tryLabel(model.name)),
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
                  ? l10n.use
                  : l10n.download2(formatModelSize(model.sizeBytes))),
            ),
            TextButton(
              key: dismissProposalKey,
              onPressed:
                  blocked ? null : () => controller.dismissProposal(proposal),
              child: Text(l10n.notNow),
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
        title: Text(l10n.suggestedForThisDevice(
            model.name, suggestion.efficient ? ', efficient mode' : '')),
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
                    ? l10n.use
                    : l10n.download2(formatModelSize(model.sizeBytes))),
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
          title: Text(l10n.efficientMode),
          subtitle: Text(l10n.runsTheSameModelsOnThe(gpuName)),
          value: efficient.enabled,
          onChanged: controller.isBusy
              ? null
              : (bool enabled) => controller.setEfficientGpu(enabled: enabled),
        ),
      if (efficient.gpuBlocked)
        ListTile(
          leading: const Icon(Icons.warning_amber_outlined),
          title: Text(l10n.theGraphicsChipStoppedTheApp),
          subtitle: Align(
            alignment: Alignment.centerLeft,
            child: TextButton(
              onPressed: controller.isBusy ? null : controller.retryGpu,
              child: Text(l10n.tryTheGraphicsChipAgain),
            ),
          ),
        ),
    ];
  }

  /// One model's row; `withHint` starts its notes with what its size is good for.
  Widget _modelTile(TranscriptionSettings settings, WhisperModelInfo model,
      {bool withHint = false}) {
    final TranscriptionSettingsController controller = widget.controller;
    final WhisperDownload download = settings.download;
    final bool downloadingThis =
        download.state == 'downloading' && download.fileName == model.fileName;
    final bool selected = settings.selectedModel == model.fileName;
    final bool wrongLanguage = model.englishOnly &&
        widget.languageCode != 'en' &&
        widget.languageCode != 'auto';
    final List<String> notes = <String>[
      if (withHint && model.name == model.family)
        if (_familyHints(widget.bundlesWhisperModel)[model.family]
            case final String hint)
          hint,
      formatModelSize(model.sizeBytes),
      if (model.onDevice) l10n.onThisDevice,
      if (settings.automatic.enabled &&
          settings.automatic.fileName == model.fileName)
        l10n.usedByAutomatic,
      if (model.englishOnly) l10n.englishOnly,
      if (model.tooLargeForMemory) l10n.mayBeTooLargeForThis,
      if (wrongLanguage) l10n.doesnTMatchYourLanguage,
      if ((settings.availableDiskBytes ?? model.sizeBytes) < model.sizeBytes &&
          !model.onDevice)
        l10n.notEnoughFreeSpace,
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
              tooltip: l10n.cancelDownload,
              icon: const Icon(Icons.close),
              onPressed: controller.cancelDownload,
            )
          else if (!model.onDevice)
            IconButton(
              tooltip: l10n.download2(model.name),
              icon: const Icon(Icons.download),
              onPressed: controller.isDownloading || controller.isBusy
                  ? null
                  : () => _download(model),
            )
          else if (model.removable)
            IconButton(
              tooltip: l10n.remove3(model.name),
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
            Text(l10n.glossary, style: Theme.of(context).textTheme.titleMedium),
            const SizedBox(height: 4),
            Text(l10n.namesProjectNamesAndTermsOne),
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
                child: Text(l10n.saveGlossary),
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
