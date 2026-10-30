// The Settings destination, on every platform (Issue #46). Android adds the App lock switch; every
// platform has Diagnostics: the Telemetry page (Issue #53), the log level and exporting the logs
// (saved through a dialog on desktops, shared through the share sheet on Android). With device sync
// on, it also chooses where recordings are transcribed (Issue #59).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/app_lock_controller.dart';
import 'package:note_taker_ui/controllers/log_settings_controller.dart';
import 'package:note_taker_ui/controllers/transcription_language_controller.dart';
import 'package:note_taker_ui/controllers/transcription_route_controller.dart';
import 'package:note_taker_ui/infrastructure/app_lock_bridge.dart';
import 'package:note_taker_ui/infrastructure/log_bridge.dart';

/// Finds the default language selector in tests.
const Key defaultLanguageSelectorKey = Key('default-language-selector');

/// Renders the settings over their controllers; it holds no logic of its own.
final class SettingsScreen extends StatelessWidget {
  /// Creates the screen.
  ///
  /// Inputs: The log settings controller, already loaded by the composition root, and the
  /// app-lock controller where the platform has App lock (Android), already initialized, and
  /// the Telemetry page, which adds a Telemetry entry that opens it, and the transcription
  /// choice, shown only when device sync is on, and the default transcription language.
  /// Outputs: A Material 3 settings list.
  /// Side effects: None; the controls ask their controllers, and Telemetry pushes its page.
  const SettingsScreen({
    required this.logSettingsController,
    this.appLockController,
    this.telemetryScreenBuilder,
    this.transcriptionRouteController,
    this.languageController,
    super.key,
  });

  final LogSettingsController logSettingsController;
  final AppLockController? appLockController;
  final WidgetBuilder? telemetryScreenBuilder;
  final TranscriptionRouteController? transcriptionRouteController;
  final TranscriptionLanguageController? languageController;

  @override
  Widget build(BuildContext context) {
    final AppLockController? appLock = appLockController;
    final TranscriptionRouteController? route = transcriptionRouteController;
    final TranscriptionLanguageController? language = languageController;
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListenableBuilder(
        listenable: Listenable.merge(<Listenable?>[
          logSettingsController,
          appLock,
          route,
          language,
        ]),
        builder: (BuildContext context, Widget? child) => ListView(
          children: <Widget>[
            if (appLock != null) ...<Widget>[
              ..._appLockTiles(context, appLock),
              const Divider(),
            ],
            if (language != null) ...<Widget>[
              ..._languageTiles(language),
              const Divider(),
            ],
            if (route != null && route.available) ...<Widget>[
              ..._transcriptionTiles(route),
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
    ];
  }

  /// Explains what the App lock switch does, or why it is unavailable.
  static String _appLockDescription(AppLockStatus? status) => switch (status) {
        null => 'Unable to read the app lock setting.',
        AppLockStatus(supported: false) => 'Needs Android 10 or later.',
        AppLockStatus(deviceSecure: false) =>
          'Set a screen lock in Android settings to use App lock.',
        _ => 'Ask for your fingerprint, face, or screen lock when you return '
            'to Note Taker after ${lockAfterBackground.inSeconds} seconds '
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
