// A transcript segment's speaker label that renames its voice (#186): double-click or long-press
// opens a dialog with the name and the saved people as suggestions. A saved person is renamed in
// every meeting; an unknown voice is named as in Voices. A name another saved person has offers
// to merge the two (#188). The widget only collects the name; Rust
// validates and stores it.
import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/speaker_rename_bridge.dart';
import 'package:note_taker_ui/infrastructure/voice_gallery_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Key of the dialog's name field, for tests.
const Key speakerNameFieldKey = ValueKey<String>('speaker-name-field');

/// Key of the dialog's Rename button, for tests.
const Key renameSpeakerButtonKey = ValueKey<String>('rename-speaker-button');

/// Key of the button offered when another saved person has the name (#188), for tests.
const Key mergeSpeakerButtonKey = ValueKey<String>('merge-speaker-button');

/// Key of the merge confirmation's button, for tests.
const Key confirmMergeSpeakerKey = ValueKey<String>('confirm-merge-speaker');

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
  List<SavedPerson> _savedPeople = const <SavedPerson>[];
  String? _error;
  bool _saving = false;

  /// The saved person who already has the typed name, offered as a merge (#188).
  SavedPerson? _takenBy;

  @override
  void initState() {
    super.initState();
    widget.renamer.savedPeople().then((List<SavedPerson> people) {
      if (mounted) {
        setState(() => _savedPeople = people);
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
    return _savedPeople
        .map((SavedPerson person) => person.displayName)
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
          _takenBy = error.code == 'name_taken' ? _personNamed(typed) : null;
          _error = switch (error.code) {
            'name_taken' => l10n.anotherSavedPersonIsCalled(
                _takenBy?.displayName ?? typed.trim()),
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

  /// The other saved person called `typed` (trimmed, ignoring case), as Rust compares names.
  SavedPerson? _personNamed(String typed) {
    final String wanted = typed.trim().toLowerCase();
    for (final SavedPerson person in _savedPeople) {
      if (person.identityId != widget.voice.id &&
          person.displayName.trim().toLowerCase() == wanted) {
        return person;
      }
    }
    return null;
  }

  /// Asks before merging this person into `kept`, then merges and closes with the kept name.
  Future<void> _merge(SavedPerson kept) async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.mergeWithName(kept.displayName)),
        content: Text(l10n.mergeWarning(widget.initialName, kept.displayName)),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            key: confirmMergeSpeakerKey,
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l10n.merge),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) {
      return;
    }
    setState(() => _saving = true);
    try {
      final String stored =
          await widget.renamer.merge(widget.voice.id, kept.identityId);
      if (mounted) {
        Navigator.of(context).pop(stored);
      }
    } on Object catch (error) {
      if (mounted) {
        setState(() {
          _saving = false;
          _error = error is SpeakerRenameException && error.code == 'recording'
              ? l10n.peopleCannotBeMergedWhileRecording
              : l10n.thesePeopleCouldNotBeMerged;
        });
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    final List<String> suggestions = _suggestions;
    final SavedPerson? takenBy =
        widget.voice.kind == SpeakerVoiceKind.person ? _takenBy : null;
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
              onChanged: (_) => setState(() {
                _error = null;
                _takenBy = null;
              }),
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
        if (takenBy != null)
          FilledButton(
            key: mergeSpeakerButtonKey,
            onPressed: _saving ? null : () => _merge(takenBy),
            child: Text(l10n.mergeWithName(takenBy.displayName)),
          )
        else
          FilledButton(
            key: renameSpeakerButtonKey,
            onPressed: _saving ? null : _save,
            child: Text(l10n.rename),
          ),
      ],
    );
  }
}
