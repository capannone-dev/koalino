// The Settings destination, on every platform (Issue #46). Android adds the App lock switch; every
// platform has Diagnostics: the Telemetry page (Issue #53), the log level and exporting the logs
// (saved through a dialog on desktops, shared through the share sheet on Android). With device sync
// on, it also chooses where recordings are transcribed (Issue #59). Android adds Back up all data
// and Restore from backup (#167). About Koalino closes the list (#115).

import 'dart:io' show Platform;

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/app_language_controller.dart';
import 'package:note_taker_ui/controllers/app_lock_controller.dart';
import 'package:note_taker_ui/controllers/network_settings_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/presentation/about_tile.dart';
import 'package:note_taker_ui/presentation/ai_settings_screen.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:note_taker_ui/controllers/transcription_route_controller.dart';
import 'package:note_taker_ui/controllers/transcription_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/app_lock_bridge.dart';
import 'package:note_taker_ui/infrastructure/log_bridge.dart';
import 'package:note_taker_ui/infrastructure/npu_download.dart';
import 'package:note_taker_ui/infrastructure/transcription_settings_bridge.dart';
import 'package:note_taker_ui/controllers/call_watch_controller.dart';
import 'package:note_taker_ui/infrastructure/assistant_access_bridge.dart';
import 'package:note_taker_ui/infrastructure/notes_folder_bridge.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/infrastructure/background_core.dart';
import 'package:note_taker_ui/infrastructure/backup_bridge.dart';
import 'package:note_taker_ui/presentation/backup_section.dart';
import 'package:note_taker_ui/presentation/recordings_widgets.dart';
import 'package:note_taker_ui/presentation/assistant_access_screen.dart';
import 'package:note_taker_ui/presentation/call_offer_banner.dart';
import 'package:note_taker_ui/presentation/notes_folder_screen.dart';
import 'package:note_taker_ui/presentation/transcription_settings_screen.dart';
import 'package:note_taker_ui/controllers/developer_settings_controller.dart';
import 'package:note_taker_ui/presentation/developer_settings_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Finds the default language selector in tests.
const Key defaultLanguageSelectorKey = Key('default-language-selector');

/// Finds the app language selector in tests (#124).
const Key appLanguageSelectorKey = Key('app-language-selector');

/// Finds the relay address field in tests.
const Key relayAddressFieldKey = ValueKey<String>('relay-address');

/// Finds the device addresses field in tests.
const Key peerAddressesFieldKey = ValueKey<String>('peer-addresses');

/// Renders the settings over their controllers; it holds no logic of its own.
final class SettingsScreen extends StatelessWidget {
  /// Creates the screen.
  ///
  /// Inputs: The log settings controller, already loaded by the composition root, and the
  /// app-lock controller where the platform has App lock (Android), already initialized, and
  /// the Telemetry page, which adds a Telemetry entry that opens it, the Devices page, which adds
  /// a Devices entry first in the list (Issue #84), and the transcription
  /// choice, shown only when device sync is on, the default transcription language, and the
  /// network addresses, also shown only when device sync is on.
  /// Outputs: A Material 3 settings list.
  /// Side effects: None; the controls ask their controllers, and Telemetry pushes its page.
  const SettingsScreen({
    required this.logSettingsController,
    this.developerSettingsController,
    this.appLanguageController,
    this.appLockController,
    this.telemetryScreenBuilder,
    this.devicesScreenBuilder,
    this.transcriptionRouteController,
    this.languageController,
    this.networkSettingsController,
    this.transcriptionSettingsBridge,
    this.summariesController,
    this.notesFolderBridge,
    this.assistantAccessBridge,
    this.callWatchController,
    this.recordingsBridge,
    this.backupBridge,
    super.key,
  });

  final LogSettingsController logSettingsController;

  /// Adds the version row and, once seven taps unlock it, Developer settings (#187) when given.
  final DeveloperSettingsController? developerSettingsController;

  /// Adds the app language selector (#124) when given.
  final AppLanguageController? appLanguageController;
  final AppLockController? appLockController;
  final WidgetBuilder? telemetryScreenBuilder;

  /// Opens pairing and the paired devices, which left the bottom bar (Issue #84).
  final WidgetBuilder? devicesScreenBuilder;
  final TranscriptionRouteController? transcriptionRouteController;
  final TranscriptionLanguageController? languageController;
  final NetworkSettingsController? networkSettingsController;

  /// Adds the Transcription page (the Whisper model, Issue #67) when given.
  final TranscriptionSettingsBridge? transcriptionSettingsBridge;

  /// Adds the AI connections page (summaries, #95) when given.
  final SummariesController? summariesController;

  /// Adds the notes folder page (#98, desktops) when given.
  final NotesFolderBridge? notesFolderBridge;

  /// Adds the AI assistant access page (#112, desktops) when given.
  final AssistantAccessBridge? assistantAccessBridge;

  /// Adds the call offer switches (#111, desktops) when given and the system can tell.
  final CallWatchController? callWatchController;

  /// Shows Keep meeting recordings and Wipe all recordings when given (#140).
  final RecordingsBridge? recordingsBridge;

  /// Shows Back up all data and Restore from backup when given (#167, Android).
  final BackupBridge? backupBridge;

  /// Everything the settings pages show, so either page rebuilds when any of it changes.
  Listenable get _changes => Listenable.merge(<Listenable?>[
        logSettingsController,
        appLockController,
        appLanguageController,
        transcriptionRouteController,
        languageController,
        networkSettingsController,
      ]);

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: Text(l10n.settings)),
      body: ListenableBuilder(
        listenable: _changes,
        builder: (BuildContext context, Widget? child) =>
            ListView(children: _basicTiles(context)),
      ),
    );
  }

  /// The settings most people change (#209): languages, keeping recordings, devices and the AI
  /// connection, then the way to everything else.
  List<Widget> _basicTiles(BuildContext context) {
    final TranscriptionLanguageController? language = languageController;
    return <Widget>[
      if (appLanguageController
          case final AppLanguageController appLanguage) ...<Widget>[
        _appLanguageTile(appLanguage),
      ],
      if (language != null) ..._languageTiles(language),
      if (recordingsBridge case final RecordingsBridge recordings) ...<Widget>[
        const Divider(),
        RecordingsSection(bridge: recordings),
      ],
      const Divider(),
      if (devicesScreenBuilder case final WidgetBuilder devicesScreen)
        ListTile(
          leading: const Icon(Icons.devices_outlined),
          title: Text(l10n.devices),
          subtitle: Text(l10n.pairYourDevicesAndSeeWhich),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => Navigator.of(context)
              .push(MaterialPageRoute<void>(builder: devicesScreen)),
        ),
      if (summariesController case final SummariesController summaries)
        ListTile(
          leading: const Icon(Icons.auto_awesome_outlined),
          title: Text(l10n.aiConnections),
          subtitle: Text(l10n.languageModelsForMeetingSummariesAnd),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
            builder: (BuildContext context) =>
                AiSettingsScreen(controller: summaries),
          )),
        ),
      const Divider(),
      ListTile(
        key: advancedSettingsKey,
        leading: const Icon(Icons.tune),
        title: Text(l10n.advancedSettings),
        subtitle: Text(l10n.advancedSettingsDescription),
        trailing: const Icon(Icons.chevron_right),
        onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
          builder: (BuildContext context) => Scaffold(
            appBar: AppBar(title: Text(l10n.advancedSettings)),
            body: ListenableBuilder(
              listenable: _changes,
              builder: (BuildContext context, Widget? child) =>
                  ListView(children: _advancedTiles(context)),
            ),
          ),
        )),
      ),
    ];
  }

  /// The rest (#209), grouped: Transcription, Storage, AI, Privacy and Developer.
  List<Widget> _advancedTiles(BuildContext context) {
    final AppLockController? appLock = appLockController;
    final TranscriptionRouteController? route = transcriptionRouteController;
    final TranscriptionLanguageController? language = languageController;
    final NetworkSettingsController? network = networkSettingsController;
    final List<Widget> transcription = <Widget>[
      if (transcriptionSettingsBridge
          case final TranscriptionSettingsBridge bridge)
        ListTile(
          leading: const Icon(Icons.record_voice_over_outlined),
          title: Text(l10n.transcriptionModel),
          subtitle: Text(l10n.chooseTheWhisperModel),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => Navigator.of(context).push(
            MaterialPageRoute<void>(
              builder: (BuildContext context) => TranscriptionSettingsPage(
                bridge: bridge,
                languageCode: language?.defaultLanguage ?? 'auto',
              ),
            ),
          ),
        ),
      if (route != null && route.available) ..._transcriptionTiles(route),
      if (callWatchController case final CallWatchController callWatch)
        CallWatchSettingsTiles(controller: callWatch),
    ];
    final List<Widget> storage = <Widget>[
      if (backupBridge case final BackupBridge backup)
        BackupSection(bridge: backup),
      if (notesFolderBridge case final NotesFolderBridge notesFolder)
        ListTile(
          leading: const Icon(Icons.folder_copy_outlined),
          title: Text(l10n.notesFolder),
          subtitle: Text(l10n.keepProjectsAndMeetingsAsFiles),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
            builder: (BuildContext context) =>
                NotesFolderScreen(bridge: notesFolder),
          )),
        ),
    ];
    final List<Widget> ai = <Widget>[
      if (assistantAccessBridge
          case final AssistantAccessBridge assistantAccess)
        ListTile(
          leading: const Icon(Icons.smart_toy_outlined),
          title: Text(l10n.aiAssistantAccess),
          subtitle: Text(l10n.letYourOwnAssistantSuchAs),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
            builder: (BuildContext context) =>
                AssistantAccessScreen(bridge: assistantAccess),
          )),
        ),
    ];
    final List<Widget> privacy = <Widget>[
      if (appLock != null) ..._appLockTiles(context, appLock),
      if (network != null && network.available)
        NetworkAddressesSection(controller: network),
    ];
    return <Widget>[
      for (final (String title, List<Widget> tiles) in <(String, List<Widget>)>[
        (l10n.transcription, transcription),
        (l10n.storage, storage),
        (l10n.aiSection, ai),
        (l10n.privacy, privacy),
        (l10n.developer, _diagnosticsTiles(context)),
      ])
        if (tiles.isNotEmpty) ...<Widget>[
          SettingsSectionHeader(title),
          ...tiles,
          const Divider(),
        ],
    ];
  }

  /// The app language selector (#124): the system's, or one of [appLanguages] by its own name.
  Widget _appLanguageTile(AppLanguageController appLanguage) => ListTile(
        leading: const Icon(Icons.language),
        title: Text(l10n.appLanguage),
        subtitle: Text(l10n.appLanguageDescription),
        trailing: DropdownButton<String?>(
          key: appLanguageSelectorKey,
          value: appLanguage.languageCode,
          underline: const SizedBox.shrink(),
          onChanged: appLanguage.choose,
          items: <DropdownMenuItem<String?>>[
            DropdownMenuItem<String?>(child: Text(l10n.systemLanguage)),
            for (final AppLanguage language in appLanguages)
              DropdownMenuItem<String?>(
                  value: language.code, child: Text(language.nativeName)),
          ],
        ),
      );

  /// The App lock switch and its message.
  List<Widget> _appLockTiles(BuildContext context, AppLockController appLock) {
    final AppLockStatus? status = appLock.status;
    final bool available = status != null &&
        status.supported &&
        status.deviceSecure &&
        !appLock.isAuthenticating;
    return <Widget>[
      SwitchListTile(
        secondary: const Icon(Icons.lock_outline),
        title: Text(l10n.appLock),
        subtitle: Text(_appLockDescription(status)),
        value: status?.locks ?? false,
        onChanged: available ? appLock.setEnabled : null,
      ),
      if (appLock.message case final String message)
        _ErrorText(message: message),
    ];
  }

  /// The transcription choice and its message.
  List<Widget> _languageTiles(TranscriptionLanguageController language) {
    return <Widget>[
      ListTile(
        leading: const Icon(Icons.translate),
        title: Text(l10n.defaultLanguage),
        subtitle: Text(l10n.usedForNewRecordingsAutoDetects),
        trailing: DropdownButton<String>(
          key: defaultLanguageSelectorKey,
          value: language.defaultLanguage,
          underline: const SizedBox.shrink(),
          menuMaxHeight: 400,
          onChanged: language.isSaving
              ? null
              : (String? chosen) {
                  if (chosen != null) {
                    language.chooseDefault(chosen);
                  }
                },
          items: <DropdownMenuItem<String>>[
            for (final LanguageChoice choice in language.choices)
              DropdownMenuItem<String>(
                  value: choice.code, child: Text(choice.label)),
          ],
        ),
      ),
      if (language.message case final String message)
        _ErrorText(message: message),
    ];
  }

  /// Where recordings are transcribed.
  List<Widget> _transcriptionTiles(TranscriptionRouteController route) {
    return <Widget>[
      ListTile(
        leading: const Icon(Icons.devices_outlined),
        title: Text(l10n.transcribeOn),
        subtitle: Text(l10n.anotherDeviceReceivesTheRecordingS),
        trailing: DropdownButton<String>(
          value: route.route,
          underline: const SizedBox.shrink(),
          onChanged: route.isSaving
              ? null
              : (String? chosen) {
                  if (chosen != null) {
                    route.choose(chosen);
                  }
                },
          items: <DropdownMenuItem<String>>[
            for (final TranscriptionRouteChoice choice in route.choices)
              DropdownMenuItem<String>(
                  value: choice.route, child: Text(choice.label)),
          ],
        ),
      ),
      if (route.message case final String message) _ErrorText(message: message),
    ];
  }

  /// The Telemetry entry, the log level selector and the export action.
  List<Widget> _diagnosticsTiles(BuildContext context) {
    final LogSettingsController logs = logSettingsController;
    final WidgetBuilder? telemetryScreen = telemetryScreenBuilder;
    return <Widget>[
      if (telemetryScreen != null)
        ListTile(
          leading: const Icon(Icons.insights_outlined),
          title: Text(l10n.telemetry),
          subtitle: Text(l10n.howLongTranscriptionTakesOnEach),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => Navigator.of(context)
              .push(MaterialPageRoute<void>(builder: telemetryScreen)),
        ),
      ListTile(
        leading: const Icon(Icons.tune),
        title: Text(l10n.logLevel),
        subtitle: Text(l10n.howMuchDetailTheAppWrites),
        trailing: DropdownButton<LogLevel>(
          value: logs.level,
          hint: Text(l10n.custom),
          underline: const SizedBox.shrink(),
          onChanged: (LogLevel? level) {
            if (level != null) {
              logs.setLevel(level);
            }
          },
          items: <DropdownMenuItem<LogLevel>>[
            for (final LogLevel level in LogLevel.values)
              DropdownMenuItem<LogLevel>(
                  value: level, child: Text(level.label)),
          ],
        ),
      ),
      if (logs.levelMessage case final String message)
        _ErrorText(message: message),
      ListTile(
        leading: Icon(
            logs.shares ? Icons.share_outlined : Icons.file_download_outlined),
        title: Text(logs.shares ? l10n.shareLogs : l10n.exportLogs),
        subtitle: Text(l10n.theLast7DaysOfDiagnostic),
        trailing: logs.isExporting
            ? const SizedBox.square(
                dimension: 24, child: CircularProgressIndicator())
            : null,
        enabled: !logs.isExporting,
        onTap: () async {
          final String? message = await logs.exportLogs();
          if (message != null && context.mounted) {
            ScaffoldMessenger.of(context)
                .showSnackBar(SnackBar(content: Text(message)));
          }
        },
      ),
      const AboutKoalinoTile(),
      if (developerSettingsController
          case final DeveloperSettingsController developer) ...<Widget>[
        DeveloperSettingsTile(controller: developer),
        DeveloperVersionTile(controller: developer, version: appVersion),
      ],
    ];
  }

  /// Explains what the App lock switch does, or why it is unavailable.
  static String _appLockDescription(AppLockStatus? status) => switch (status) {
        null => l10n.unableToReadTheAppLock,
        AppLockStatus(supported: false) => l10n.needsAndroid10OrLater,
        AppLockStatus(deviceSecure: false) => l10n.setAScreenLockInAndroid,
        _ => l10n.askForYourFingerprintFaceOr(lockAfterBackground.inSeconds),
      };
}

/// A setting's error message, in the theme's error colour.
/// The Advanced entry at the end of the basic settings (#209).
const Key advancedSettingsKey = ValueKey<String>('advanced-settings');

/// The title of a group of settings, such as Transcription or Privacy (#209).
final class SettingsSectionHeader extends StatelessWidget {
  /// Creates the header with `title`.
  const SettingsSectionHeader(this.title, {super.key});

  final String title;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Semantics(
      header: true,
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 16, 16, 4),
        child: Text(title,
            style: theme.textTheme.titleSmall
                ?.copyWith(color: theme.colorScheme.primary)),
      ),
    );
  }
}

final class _ErrorText extends StatelessWidget {
  const _ErrorText({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16),
        child: Text(message,
            style: TextStyle(color: Theme.of(context).colorScheme.error)),
      );
}

/// The relay and static peer fields (Issue #64). It only holds the typed text; the controller does
/// the rest.
final class NetworkAddressesSection extends StatefulWidget {
  /// Creates the section over `controller`, which has loaded.
  const NetworkAddressesSection({required this.controller, super.key});

  /// Reads and saves the addresses.
  final NetworkSettingsController controller;

  @override
  State<NetworkAddressesSection> createState() =>
      _NetworkAddressesSectionState();
}

/// Holds the fields' text, filled from what is in effect.
final class _NetworkAddressesSectionState
    extends State<NetworkAddressesSection> {
  late final TextEditingController _relayField =
      TextEditingController(text: widget.controller.relayAddress);
  late final TextEditingController _peersField =
      TextEditingController(text: widget.controller.peerAddresses);

  @override
  void dispose() {
    _relayField.dispose();
    _peersField.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final NetworkSettingsController controller = widget.controller;
    return Padding(
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(l10n.otherNetworks,
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 4),
          Text(l10n.aRelayOnYourOwnAlways),
          const SizedBox(height: 12),
          TextField(
            key: relayAddressFieldKey,
            controller: _relayField,
            decoration: InputDecoration(
              labelText: l10n.relayAddress,
              hintText: '/ip4/203.0.113.5/tcp/4001/p2p/12D3Koo…',
              border: OutlineInputBorder(),
            ),
          ),
          if (controller.relayStatus case final String relayStatus)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(relayStatus),
            ),
          const SizedBox(height: 12),
          TextField(
            key: peerAddressesFieldKey,
            controller: _peersField,
            minLines: 2,
            maxLines: 5,
            decoration: InputDecoration(
              labelText: l10n.deviceAddressesOptionalOnePerLine,
              border: OutlineInputBorder(),
            ),
          ),
          const SizedBox(height: 12),
          FilledButton(
            onPressed: controller.isSaving
                ? null
                : () => controller.save(
                      relay: _relayField.text,
                      peers: _peersField.text,
                    ),
            child: Text(l10n.saveAddresses),
          ),
          if (controller.message case final String message)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Text(message),
            ),
        ],
      ),
    );
  }
}

/// Owns the page's controller for as long as the page is open; downloads continue in the core.
/// Settings opens it, and so does a recording error a model choice can fix (#203).
final class TranscriptionSettingsPage extends StatefulWidget {
  /// Inputs: the core's transcription settings `bridge`; the default meeting `languageCode`.
  const TranscriptionSettingsPage(
      {required this.bridge, required this.languageCode, super.key});

  final TranscriptionSettingsBridge bridge;
  final String languageCode;

  @override
  State<TranscriptionSettingsPage> createState() =>
      _TranscriptionSettingsPageState();
}

/// Creates and disposes the controller.
final class _TranscriptionSettingsPageState
    extends State<TranscriptionSettingsPage> {
  late final TranscriptionSettingsController _controller =
      TranscriptionSettingsController(
          bridge: widget.bridge, npuDownloader: platformNpuDownloader());

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => TranscriptionSettingsScreen(
        controller: _controller,
        languageCode: widget.languageCode,
        // A phone runs the pass from a charging job while the app is closed (#87).
        backgroundPass:
            Platform.isAndroid ? const BackgroundPassBridge() : null,
      );
}
