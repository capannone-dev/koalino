// The Settings destination, on every platform (Issue #46). Android adds the App lock switch; every
// platform has Diagnostics: the Telemetry page (Issue #53), the log level and exporting the logs
// (saved through a dialog on desktops, shared through the share sheet on Android). With device sync
// on, it also chooses where recordings are transcribed (Issue #59). About Koalino closes the list (#115).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
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
import 'package:note_taker_ui/presentation/assistant_access_screen.dart';
import 'package:note_taker_ui/presentation/call_offer_banner.dart';
import 'package:note_taker_ui/presentation/notes_folder_screen.dart';
import 'package:note_taker_ui/presentation/transcription_settings_screen.dart';

/// Finds the default language selector in tests.
const Key defaultLanguageSelectorKey = Key('default-language-selector');

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
    super.key,
  });

  final LogSettingsController logSettingsController;
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

  @override
  Widget build(BuildContext context) {
    final AppLockController? appLock = appLockController;
    final TranscriptionRouteController? route = transcriptionRouteController;
    final TranscriptionLanguageController? language = languageController;
    final NetworkSettingsController? network = networkSettingsController;
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListenableBuilder(
        listenable: Listenable.merge(<Listenable?>[
          logSettingsController,
          appLock,
          route,
          language,
          network,
        ]),
        builder: (BuildContext context, Widget? child) => ListView(
          children: <Widget>[
            if (devicesScreenBuilder
                case final WidgetBuilder devicesScreen) ...<Widget>[
              ListTile(
                leading: const Icon(Icons.devices_outlined),
                title: const Text('Devices'),
                subtitle: const Text(
                    'Pair your devices and see which ones are connected.'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context)
                    .push(MaterialPageRoute<void>(builder: devicesScreen)),
              ),
              const Divider(),
            ],
            if (appLock != null) ...<Widget>[
              ..._appLockTiles(context, appLock),
              const Divider(),
            ],
            if (language != null) ...<Widget>[
              ..._languageTiles(language),
              const Divider(),
            ],
            if (transcriptionSettingsBridge != null) ...<Widget>[
              ListTile(
                leading: const Icon(Icons.record_voice_over_outlined),
                title: const Text('Transcription model'),
                subtitle: const Text('Choose the Whisper model.'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute<void>(
                    builder: (BuildContext context) => _TranscriptionPage(
                      bridge: transcriptionSettingsBridge!,
                      languageCode: language?.defaultLanguage ?? 'auto',
                    ),
                  ),
                ),
              ),
              const Divider(),
            ],
            if (summariesController
                case final SummariesController summaries) ...<Widget>[
              ListTile(
                leading: const Icon(Icons.auto_awesome_outlined),
                title: const Text('AI connections'),
                subtitle: const Text(
                    'Language models for meeting summaries, and the summary prompt.'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                  builder: (BuildContext context) =>
                      AiSettingsScreen(controller: summaries),
                )),
              ),
              const Divider(),
            ],
            if (notesFolderBridge
                case final NotesFolderBridge notesFolder) ...<Widget>[
              ListTile(
                leading: const Icon(Icons.folder_copy_outlined),
                title: const Text('Notes folder'),
                subtitle: const Text(
                    'Keep projects and meetings as files you can edit with any editor.'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                  builder: (BuildContext context) =>
                      NotesFolderScreen(bridge: notesFolder),
                )),
              ),
              const Divider(),
            ],
            if (assistantAccessBridge
                case final AssistantAccessBridge assistantAccess) ...<Widget>[
              ListTile(
                leading: const Icon(Icons.smart_toy_outlined),
                title: const Text('AI assistant access'),
                subtitle: const Text(
                    'Let your own assistant, such as Claude Code, read your notes.'),
                trailing: const Icon(Icons.chevron_right),
                onTap: () => Navigator.of(context).push(MaterialPageRoute<void>(
                  builder: (BuildContext context) =>
                      AssistantAccessScreen(bridge: assistantAccess),
                )),
              ),
              const Divider(),
            ],
            if (callWatchController case final CallWatchController callWatch)
              CallWatchSettingsTiles(controller: callWatch),
            if (route != null && route.available) ...<Widget>[
              ..._transcriptionTiles(route),
              const Divider(),
            ],
            if (network != null && network.available) ...<Widget>[
              NetworkAddressesSection(controller: network),
              const Divider(),
            ],
            ..._diagnosticsTiles(context),
          ],
        ),
      ),
    );
  }

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
        title: const Text('App lock'),
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
        title: const Text('Default language'),
        subtitle: const Text('Used for new recordings. '
            'Auto detects the language as people speak.'),
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
        title: const Text('Transcribe on'),
        subtitle: const Text('Another device receives the recording\'s audio, '
            'encrypted, and deletes it once transcribed. '
            'If it is unavailable, this device transcribes instead.'),
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
          title: const Text('Telemetry'),
          subtitle: const Text('How long transcription takes on each device, '
              'and its estimated energy use.'),
          trailing: const Icon(Icons.chevron_right),
          onTap: () => Navigator.of(context)
              .push(MaterialPageRoute<void>(builder: telemetryScreen)),
        ),
      ListTile(
        leading: const Icon(Icons.tune),
        title: const Text('Log level'),
        subtitle: const Text('How much detail the app writes to its logs. '
            'Debug and Trace help diagnose problems but write more.'),
        trailing: DropdownButton<LogLevel>(
          value: logs.level,
          hint: const Text('Custom'),
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
        title: Text(logs.shares ? 'Share logs' : 'Export logs'),
        subtitle: const Text('The last 7 days of diagnostic logs, '
            'for example to attach to a bug report.'),
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
    ];
  }

  /// Explains what the App lock switch does, or why it is unavailable.
  static String _appLockDescription(AppLockStatus? status) => switch (status) {
        null => 'Unable to read the app lock setting.',
        AppLockStatus(supported: false) => 'Needs Android 10 or later.',
        AppLockStatus(deviceSecure: false) =>
          'Set a screen lock in Android settings to use App lock.',
        _ => 'Ask for your fingerprint, face, or screen lock when you return '
            'to Koalino after ${lockAfterBackground.inSeconds} seconds '
            'away. Recording and sync keep running while it is locked.',
      };
}

/// A setting's error message, in the theme's error colour.
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
          Text('Other networks',
              style: Theme.of(context).textTheme.titleMedium),
          const SizedBox(height: 4),
          const Text(
              'A relay on your own always-on device lets your devices reach each other away '
              'from the same Wi-Fi. Syncing on the same network keeps working without it.'),
          const SizedBox(height: 12),
          TextField(
            key: relayAddressFieldKey,
            controller: _relayField,
            decoration: const InputDecoration(
              labelText: 'Relay address',
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
            decoration: const InputDecoration(
              labelText: 'Device addresses (optional, one per line)',
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
            child: const Text('Save addresses'),
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
final class _TranscriptionPage extends StatefulWidget {
  const _TranscriptionPage({required this.bridge, required this.languageCode});

  final TranscriptionSettingsBridge bridge;
  final String languageCode;

  @override
  State<_TranscriptionPage> createState() => _TranscriptionPageState();
}

/// Creates and disposes the controller.
final class _TranscriptionPageState extends State<_TranscriptionPage> {
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
      );
}
