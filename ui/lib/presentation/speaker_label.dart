// A transcript segment's speaker label that renames its voice (#186): double-click or long-press
// opens a dialog with the name and the saved people as suggestions. A saved person is renamed in
// every meeting; an unknown voice is named as in Voices. The widget only collects the name; Rust
// validates and stores it.
import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/speaker_rename_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Key of the dialog's name field, for tests.
const Key speakerNameFieldKey = ValueKey<String>('speaker-name-field');

/// Key of the dialog's Rename button, for tests.
const Key renameSpeakerButtonKey = ValueKey<String>('rename-speaker-button');

/// Shows `text` and, when the segment has a voice, renames it on double-click or long-press.
final class SpeakerLabel extends StatelessWidget {
  /// Creates a label.
  ///
  /// Inputs: The shown `text` (name and time), the current `name`, the segment's `voice` (none:
  /// the label does not react), the `renamer`, and `onRenamed`, called with the stored name.
  /// Outputs: The label widget. Side effects: None until the owner renames.
  const SpeakerLabel({
    required this.text,
    required this.name,
    required this.voice,
    required this.renamer,
    required this.onRenamed,
    this.style,
    super.key,
  });

  /// What the label shows, such as `Anna · 0:05`.
  final String text;

  /// The speaker's current name, without marks such as `(?)`.
  final String name;

  /// The voice the label renames.
  final SpeakerVoice? voice;

  /// Stores the new name.
  final SpeakerRenamer renamer;

  /// Called after a rename, so the meeting reloads.
  final ValueChanged<String> onRenamed;

  /// The label's text style.
  final TextStyle? style;

  @override
  Widget build(BuildContext context) {
    final Widget label = Text(text, style: style);
    final SpeakerVoice? renamable = voice;
    if (renamable == null) {
      return label;
    }
    return Tooltip(
      message: l10n.doubleClickOrLongPressToRename,
      waitDuration: const Duration(milliseconds: 800),
      child: MouseRegion(
        cursor: SystemMouseCursors.click,
        child: GestureDetector(
          behavior: HitTestBehavior.opaque,
          onDoubleTap: () => _rename(context, renamable),
          onLongPress: () => _rename(context, renamable),
          child: Semantics(
            button: true,
            hint: l10n.renameSpeaker,
            child: label,
          ),
        ),
      ),
    );
  }

  /// Opens the dialog and reports the stored name.
  Future<void> _rename(BuildContext context, SpeakerVoice renamable) async {
    final String? stored = await showDialog<String>(
      context: context,
      builder: (BuildContext context) => _RenameSpeakerDialog(
        voice: renamable,
        // An unknown voice's "Speaker 2" is a placeholder, not a name to edit.
        initialName: renamable.kind == SpeakerVoiceKind.person ? name : '',
        renamer: renamer,
      ),
    );
    if (stored != null) {
      onRenamed(stored);
    }
  }
}

/// Asks for the name, suggesting saved people, and stays open with the reason when refused.
final class _RenameSpeakerDialog extends StatefulWidget {
  const _RenameSpeakerDialog({
    required this.voice,
    required this.initialName,
    required this.renamer,
  });

  final SpeakerVoice voice;
  final String initialName;
  final SpeakerRenamer renamer;

  @override
  State<_RenameSpeakerDialog> createState() => _RenameSpeakerDialogState();
}

final class _RenameSpeakerDialogState extends State<_RenameSpeakerDialog> {
  late final TextEditingController _name =
      TextEditingController(text: widget.initialName);
  List<String> _savedNames = const <String>[];
  String? _error;
  bool _saving = false;

  @override
  void initState() {
    super.initState();
    widget.renamer.savedNames().then((List<String> names) {
      if (mounted) {
        setState(() => _savedNames = names);
      }
    }, onError: (Object _) {
      // Suggestions are optional; typing a name still works.
    });
  }

  @override
  void dispose() {
    _name.dispose();
    super.dispose();
  }

  /// Saved names containing what is typed, other than the current one.
  List<String> get _suggestions {
    final String typed = _name.text.trim().toLowerCase();
    if (typed.isEmpty) {
      return const <String>[];
    }
    return _savedNames
        .where((String saved) =>
            saved.toLowerCase().contains(typed) && saved.toLowerCase() != typed)
        .take(5)
        .toList(growable: false);
  }

  Future<void> _save() async {
    final String typed = _name.text;
    if (typed.trim().isEmpty) {
      setState(() => _error = l10n.useOneTo100CharactersOnOneLine);
      return;
    }
    setState(() {
      _saving = true;
      _error = null;
    });
    try {
      final String stored = await widget.renamer.rename(widget.voice, typed);
      if (mounted) {
        Navigator.of(context).pop(stored);
      }
    } on SpeakerRenameException catch (error) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = switch (error.code) {
            'name_taken' => l10n.anotherSavedPersonIsCalled(typed.trim()),
            'invalid_name' ||
            'invalid_argument' =>
              l10n.useOneTo100CharactersOnOneLine,
            _ => l10n.theNameCouldNotBeSaved,
          };
        });
      }
    } on Object {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = l10n.theNameCouldNotBeSaved;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final List<String> suggestions = _suggestions;
    return AlertDialog(
      title: Text(widget.voice.kind == SpeakerVoiceKind.person
          ? l10n.renameSpeaker
          : l10n.nameThisVoice),
      content: SizedBox(
        width: 360,
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            TextField(
              key: speakerNameFieldKey,
              controller: _name,
              autofocus: true,
              maxLength: 100,
              enabled: !_saving,
              decoration: InputDecoration(
                labelText: l10n.name,
                errorText: _error,
                errorMaxLines: 3,
              ),
              onChanged: (_) => setState(() => _error = null),
              onSubmitted: (_) => _save(),
            ),
            for (final String suggestion in suggestions)
              ListTile(
                dense: true,
                contentPadding: EdgeInsets.zero,
                leading: const Icon(Icons.person_outline),
                title: Text(suggestion),
                onTap: () => setState(() {
                  _name.text = suggestion;
                  _name.selection =
                      TextSelection.collapsed(offset: suggestion.length);
                }),
              ),
            const SizedBox(height: 8),
            Text(
              l10n.theNewNameAppliesToEveryPassage,
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: _saving ? null : () => Navigator.of(context).pop(),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          key: renameSpeakerButtonKey,
          onPressed: _saving ? null : _save,
          child: Text(l10n.rename),
        ),
      ],
    );
  }
}
