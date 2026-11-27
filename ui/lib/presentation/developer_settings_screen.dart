// Settings → Developer (#187): the version row that unlocks the section after seven taps, the
// Developer entry shown once it is unlocked, and the page that lists the app variables an expert
// can override. The core validates and saves each value for the next start; the page shows what
// is in effect now, what is saved, and asks for a restart when they differ.
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/developer_settings_controller.dart';
import 'package:note_taker_ui/infrastructure/developer_settings_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Keys for tests.
const Key developerVersionTileKey = ValueKey<String>('developer-version-tile');
const Key developerSettingsTileKey =
    ValueKey<String>('developer-settings-tile');
const Key restartNeededKey = ValueKey<String>('developer-restart-needed');
const Key developerValueFieldKey = ValueKey<String>('developer-value-field');
const Key saveDeveloperValueKey = ValueKey<String>('developer-save-value');
const Key useDefaultDeveloperValueKey =
    ValueKey<String>('developer-use-default');
const Key turnOffDeveloperSettingsKey = ValueKey<String>('developer-turn-off');
const Key confirmTurnOffDeveloperSettingsKey =
    ValueKey<String>('developer-confirm-turn-off');

/// The Settings row showing the app's version; seven taps unlock the developer section.
///
/// The countdown shows in the row itself: the row is the last in Settings, so a snack bar would
/// cover it and catch the remaining taps.
final class DeveloperVersionTile extends StatefulWidget {
  /// Creates the row for `version`.
  const DeveloperVersionTile(
      {required this.controller, required this.version, super.key});

  final DeveloperSettingsController controller;
  final String version;

  @override
  State<DeveloperVersionTile> createState() => _DeveloperVersionTileState();
}

final class _DeveloperVersionTileState extends State<DeveloperVersionTile> {
  /// The countdown, or "on" once unlocked by these taps; null before.
  String? _hint;

  Future<void> _tap() async {
    final bool wasUnlocked = widget.controller.unlocked;
    final int remaining = await widget.controller.tapVersion();
    if (!mounted || wasUnlocked) {
      return;
    }
    // Like Android's Build number: quiet for the first taps, then a countdown.
    setState(() => _hint = switch (remaining) {
          0 => l10n.developerSettingsAreOn,
          <= 4 => l10n.tapsLeftToTurnOnDeveloperSettings(remaining),
          _ => null,
        });
  }

  @override
  Widget build(BuildContext context) {
    final String? hint = _hint;
    return ListTile(
      key: developerVersionTileKey,
      leading: const Icon(Icons.tag),
      title: Text(l10n.versionNumber(widget.version)),
      subtitle:
          hint == null ? null : Semantics(liveRegion: true, child: Text(hint)),
      onTap: _tap,
    );
  }
}

/// The Developer entry, shown only once the section is unlocked.
final class DeveloperSettingsTile extends StatelessWidget {
  /// Creates the entry.
  const DeveloperSettingsTile({required this.controller, super.key});

  final DeveloperSettingsController controller;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: controller,
        builder: (BuildContext context, Widget? child) => controller.unlocked
            ? ListTile(
                key: developerSettingsTileKey,
                leading: const Icon(Icons.developer_mode),
                title: Text(l10n.developerSettings),
                subtitle: Text(l10n.appVariablesForExpertsAndDevelopers),
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute<void>(
                    builder: (BuildContext context) =>
                        DeveloperSettingsScreen(controller: controller),
                  ),
                ),
              )
            : const SizedBox.shrink(),
      );
}

/// Lists the overridable app variables.
final class DeveloperSettingsScreen extends StatelessWidget {
  /// Creates the page.
  const DeveloperSettingsScreen({required this.controller, super.key});

  final DeveloperSettingsController controller;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(
          title: Text(l10n.developerSettings),
          actions: <Widget>[
            TextButton(
              onPressed: () => _run(context, controller.reset),
              child: Text(l10n.resetAll),
            ),
          ],
        ),
        body: ListenableBuilder(
          listenable: controller,
          builder: (BuildContext context, Widget? child) {
            final ThemeData theme = Theme.of(context);
            final DeveloperSettingsState? state = controller.state;
            return ListView(
              children: <Widget>[
                Padding(
                  padding: const EdgeInsets.fromLTRB(16, 16, 16, 8),
                  child: Text(l10n.developerSettingsWarning,
                      style: theme.textTheme.bodyMedium),
                ),
                if (state?.restartNeeded ?? false)
                  Card(
                    key: restartNeededKey,
                    color: theme.colorScheme.tertiaryContainer,
                    margin:
                        const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
                    child: ListTile(
                      leading: const Icon(Icons.restart_alt),
                      title: Text(l10n.restartKoalinoToApply),
                    ),
                  ),
                for (final DeveloperSetting setting
                    in state?.settings ?? const <DeveloperSetting>[])
                  _settingTile(context, theme, setting),
                const Divider(),
                ListTile(
                  key: turnOffDeveloperSettingsKey,
                  leading: Icon(Icons.power_settings_new,
                      color: theme.colorScheme.error),
                  title: Text(l10n.turnOffDeveloperSettings),
                  onTap: () => _turnOff(context),
                ),
              ],
            );
          },
        ),
      );

  Widget _settingTile(
      BuildContext context, ThemeData theme, DeveloperSetting setting) {
    final String? saved = setting.saved;
    return ListTile(
      isThreeLine: true,
      title: Text(setting.name,
          style: theme.textTheme.titleSmall?.copyWith(
              fontFamily: 'monospace',
              fontFamilyFallback: const <String>['Courier'])),
      subtitle: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: <Widget>[
          Text(setting.description),
          Text(l10n.nowAndDefault(
              setting.inEffect ?? l10n.notSet, setting.defaultValue)),
          if (saved != null)
            Text(l10n.savedForTheNextStart(saved),
                style: TextStyle(color: theme.colorScheme.primary)),
        ],
      ),
      onTap: () => showDialog<void>(
        context: context,
        builder: (BuildContext context) =>
            _EditValueDialog(controller: controller, setting: setting),
      ),
    );
  }

  /// Runs a command, showing a snack bar when the core cannot save.
  static Future<void> _run(
      BuildContext context, Future<void> Function() command) async {
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    try {
      await command();
    } on DeveloperSettingsException catch (error) {
      messenger.showSnackBar(SnackBar(content: Text(error.message)));
    }
  }

  Future<void> _turnOff(BuildContext context) async {
    final NavigatorState navigator = Navigator.of(context);
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.turnOffDeveloperSettings),
        content: Text(l10n.turnOffDeveloperSettingsExplanation),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            key: confirmTurnOffDeveloperSettingsKey,
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l10n.turnOff),
          ),
        ],
      ),
    );
    if (confirmed != true || !context.mounted) {
      return;
    }
    await _run(context, controller.lock);
    if (!controller.unlocked) {
      navigator.pop();
    }
  }
}

/// Edits one value: options for flags and choices, a field otherwise.
final class _EditValueDialog extends StatefulWidget {
  const _EditValueDialog({required this.controller, required this.setting});

  final DeveloperSettingsController controller;
  final DeveloperSetting setting;

  @override
  State<_EditValueDialog> createState() => _EditValueDialogState();
}

final class _EditValueDialogState extends State<_EditValueDialog> {
  late final TextEditingController _field = TextEditingController(
      text: widget.setting.saved ?? widget.setting.inEffect ?? '');
  late String? _choice = widget.setting.saved ?? widget.setting.inEffect;
  String? _error;
  bool _saving = false;

  /// The words offered as buttons, or none for free text.
  List<String> get _options => switch (widget.setting.kind) {
        DeveloperSettingKind.flag => const <String>['true', 'false'],
        DeveloperSettingKind.choice => widget.setting.options,
        _ => const <String>[],
      };

  @override
  void dispose() {
    _field.dispose();
    super.dispose();
  }

  Future<void> _save(String? value) async {
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      await widget.controller.set(widget.setting.name, value);
      if (mounted) {
        Navigator.of(context).pop();
      }
    } on DeveloperSettingsException catch (error) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = error.message;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final List<String> options = _options;
    return AlertDialog(
      title: Text(widget.setting.name),
      content: SizedBox(
        width: 400,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            Text(widget.setting.description),
            const SizedBox(height: 12),
            if (options.isEmpty)
              TextField(
                key: developerValueFieldKey,
                controller: _field,
                autofocus: true,
                enabled: !_saving,
                keyboardType:
                    widget.setting.kind == DeveloperSettingKind.integer
                        ? TextInputType.number
                        : TextInputType.text,
                decoration: InputDecoration(
                  hintText: widget.setting.defaultValue,
                  errorText: _error,
                  errorMaxLines: 3,
                ),
                onSubmitted: (_) => _save(_field.text),
              )
            else ...<Widget>[
              Wrap(
                spacing: 8,
                children: <Widget>[
                  for (final String option in options)
                    ChoiceChip(
                      label: Text(option),
                      selected: _choice == option,
                      onSelected: _saving
                          ? null
                          : (_) => setState(() => _choice = option),
                    ),
                ],
              ),
              if (_error case final String error)
                Text(error,
                    style:
                        TextStyle(color: Theme.of(context).colorScheme.error)),
            ],
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          key: useDefaultDeveloperValueKey,
          onPressed: _saving ? null : () => _save(null),
          child: Text(l10n.useDefault),
        ),
        TextButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          key: saveDeveloperValueKey,
          onPressed: _saving || (options.isNotEmpty && _choice == null)
              ? null
              : () => _save(options.isEmpty ? _field.text : _choice),
          child: Text(l10n.save),
        ),
      ],
    );
  }
}
