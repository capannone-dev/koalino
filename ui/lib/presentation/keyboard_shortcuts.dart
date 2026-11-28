// Keyboard shortcuts for power users (#215): a handler that binds keys while a screen is shown, the
// Ctrl/Cmd+K command palette, and the cheat sheet that `?` opens.
//
// Single keys (R, /, Space, J, L, ?) never fire while the owner types in a text field, and only
// on the page in front, so they cannot surprise anyone typing a note or a title.

import 'dart:io' show Platform;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Whether the keyboard focus is in a text field, where single keys are text.
bool _typing() =>
    FocusManager.instance.primaryFocus?.context
        ?.findAncestorWidgetOfExactType<EditableText>() !=
    null;

/// Whether `activator` needs Control, Meta or Alt, so it is not text even in a field.
bool _withModifier(ShortcutActivator activator) => switch (activator) {
      SingleActivator(:final bool control, :final bool meta, :final bool alt) =>
        control || meta || alt,
      CharacterActivator(
        :final bool control,
        :final bool meta,
        :final bool alt
      ) =>
        control || meta || alt,
      _ => false,
    };

/// The palette's activator on this platform: Cmd+K on Apple systems, Ctrl+K elsewhere.
ShortcutActivator get commandPaletteActivator =>
    Platform.isMacOS || Platform.isIOS
        ? const SingleActivator(LogicalKeyboardKey.keyK, meta: true)
        : const SingleActivator(LogicalKeyboardKey.keyK, control: true);

/// Binds `bindings` while this widget is mounted and its page is the one in front.
///
/// It listens to the hardware keyboard rather than the focus tree, so the keys work wherever the
/// focus is (or when nothing has it), except single keys while typing in a text field.
final class AppShortcuts extends StatefulWidget {
  /// Creates the handler around `child`.
  const AppShortcuts({required this.bindings, required this.child, super.key});

  /// What each key does.
  final Map<ShortcutActivator, VoidCallback> bindings;
  final Widget child;

  @override
  State<AppShortcuts> createState() => _AppShortcutsState();
}

final class _AppShortcutsState extends State<AppShortcuts> {
  @override
  void initState() {
    super.initState();
    HardwareKeyboard.instance.addHandler(_onKey);
  }

  @override
  void dispose() {
    HardwareKeyboard.instance.removeHandler(_onKey);
    super.dispose();
  }

  /// Runs the binding `event` matches; returns whether it ran one.
  bool _onKey(KeyEvent event) {
    if (event is! KeyDownEvent || !mounted) {
      return false;
    }
    // Only the page in front, and not behind a dialog.
    if (!(ModalRoute.of(context)?.isCurrent ?? true)) {
      return false;
    }
    final bool typing = _typing();
    for (final MapEntry<ShortcutActivator, VoidCallback> binding
        in widget.bindings.entries) {
      if (typing && !_withModifier(binding.key)) {
        continue;
      }
      if (binding.key.accepts(event, HardwareKeyboard.instance)) {
        binding.value();
        return true;
      }
    }
    return false;
  }

  @override
  Widget build(BuildContext context) => widget.child;
}

/// One thing the command palette can do.
typedef PaletteCommand = ({
  IconData icon,
  String label,
  String? shortcut,
  VoidCallback run,
});

/// The command palette's search field, for tests.
const Key commandPaletteFieldKey = ValueKey<String>('command-palette-field');

/// Opens the command palette: a field that filters `commands` as the owner types; Enter runs the
/// first match, a click runs any.
///
/// Side effects: Shows a dialog; runs the chosen command after it closes.
Future<void> showCommandPalette(
    BuildContext context, List<PaletteCommand> commands) async {
  final PaletteCommand? chosen = await showDialog<PaletteCommand>(
    context: context,
    builder: (BuildContext context) => _CommandPalette(commands: commands),
  );
  chosen?.run();
}

final class _CommandPalette extends StatefulWidget {
  const _CommandPalette({required this.commands});

  final List<PaletteCommand> commands;

  @override
  State<_CommandPalette> createState() => _CommandPaletteState();
}

final class _CommandPaletteState extends State<_CommandPalette> {
  String _query = '';

  List<PaletteCommand> get _matches {
    final String query = _query.trim().toLowerCase();
    return <PaletteCommand>[
      for (final PaletteCommand command in widget.commands)
        if (query.isEmpty || command.label.toLowerCase().contains(query))
          command,
    ];
  }

  @override
  Widget build(BuildContext context) {
    final List<PaletteCommand> matches = _matches;
    return Dialog(
      alignment: const Alignment(0, -0.6),
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 480, maxHeight: 420),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Padding(
              padding: const EdgeInsets.all(12),
              child: TextField(
                key: commandPaletteFieldKey,
                autofocus: true,
                decoration: InputDecoration(
                  prefixIcon: const Icon(Icons.search),
                  hintText: l10n.typeACommand,
                  border: const OutlineInputBorder(),
                ),
                onChanged: (String value) => setState(() => _query = value),
                onSubmitted: (_) {
                  if (matches.isNotEmpty) {
                    Navigator.of(context).pop(matches.first);
                  }
                },
              ),
            ),
            Flexible(
              child: matches.isEmpty
                  ? Padding(
                      padding: const EdgeInsets.all(16),
                      child: Text(l10n.noMatchingCommands),
                    )
                  : ListView(
                      shrinkWrap: true,
                      children: <Widget>[
                        for (final PaletteCommand command in matches)
                          ListTile(
                            leading: Icon(command.icon),
                            title: Text(command.label),
                            trailing: switch (command.shortcut) {
                              final String keys => _KeyCap(keys),
                              null => null,
                            },
                            onTap: () => Navigator.of(context).pop(command),
                          ),
                      ],
                    ),
            ),
          ],
        ),
      ),
    );
  }
}

/// Shows the keyboard shortcuts (`?`).
///
/// Side effects: Shows a dialog.
Future<void> showKeyboardShortcuts(BuildContext context) {
  final String palette = Platform.isMacOS || Platform.isIOS ? '⌘ K' : 'Ctrl K';
  final List<(String, String)> rows = <(String, String)>[
    (palette, l10n.openTheCommandPalette),
    ('R', l10n.startRecording),
    ('/', l10n.searchMeetings),
    (l10n.spaceKey, l10n.playOrPauseTheRecording),
    ('J  L', l10n.back10SecondsForward10Seconds),
    ('?', l10n.showTheseShortcuts),
  ];
  return showDialog<void>(
    context: context,
    builder: (BuildContext context) => AlertDialog(
      title: Text(l10n.keyboardShortcuts),
      content: SizedBox(
        width: 400,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            for (final (String keys, String what) in rows)
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 6),
                child: Row(
                  children: <Widget>[
                    SizedBox(width: 96, child: _KeyCap(keys)),
                    const SizedBox(width: 12),
                    Expanded(child: Text(what)),
                  ],
                ),
              ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: Text(l10n.close),
        ),
      ],
    ),
  );
}

/// Keys drawn as a small key cap.
final class _KeyCap extends StatelessWidget {
  const _KeyCap(this.keys);

  final String keys;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Align(
      alignment: AlignmentDirectional.centerStart,
      widthFactor: 1,
      child: DecoratedBox(
        decoration: BoxDecoration(
          color: theme.colorScheme.surfaceContainerHighest,
          borderRadius: BorderRadius.circular(6),
          border: Border.all(color: theme.colorScheme.outlineVariant),
        ),
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
          child: Text(keys, style: theme.textTheme.labelMedium),
        ),
      ),
    );
  }
}
