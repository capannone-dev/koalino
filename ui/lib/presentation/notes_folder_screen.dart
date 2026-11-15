// The notes folder page (#98), in Settings on desktops: turns on a folder of plain files that
// mirrors projects and meetings, warns that it is a decrypted copy, and shows the last round.

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/notes_folder_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Finds the folder field in tests.
const Key notesFolderPathFieldKey = ValueKey<String>('notes-folder-path');

/// Picks a folder, or null when cancelled; replaced in tests.
typedef FolderPicker = Future<String?> Function();

/// Opens the system's folder dialog.
Future<String?> pickFolderWithDialog() =>
    FilePicker.platform.getDirectoryPath(dialogTitle: l10n.notesFolder);

/// Shows and changes the notes folder.
final class NotesFolderScreen extends StatefulWidget {
  /// Creates the page.
  const NotesFolderScreen(
      {required this.bridge,
      this.pickFolder = pickFolderWithDialog,
      super.key});

  final NotesFolderBridge bridge;
  final FolderPicker pickFolder;

  @override
  State<NotesFolderScreen> createState() => _NotesFolderScreenState();
}

/// Keeps the status, the folder being typed and the error.
final class _NotesFolderScreenState extends State<NotesFolderScreen> {
  final TextEditingController _path = TextEditingController();
  NotesFolderStatus? _status;
  String? _error;
  bool _busy = true;

  @override
  void initState() {
    super.initState();
    _run(widget.bridge.status);
  }

  @override
  void dispose() {
    _path.dispose();
    super.dispose();
  }

  /// Runs a command and shows its status or error. Side effects: One core call.
  Future<void> _run(Future<NotesFolderStatus> Function() command) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    NotesFolderStatus? status;
    String? error;
    try {
      status = await command();
    } on NotesFolderException catch (failure) {
      error = failure.message;
    }
    if (!mounted) {
      return;
    }
    setState(() {
      _busy = false;
      _error = error;
      if (status != null) {
        _status = status;
        if (_path.text.isEmpty || status.path != null) {
          _path.text = status.path ?? status.suggestedPath ?? '';
        }
      }
    });
  }

  Future<void> _choose() async {
    final String? picked = await widget.pickFolder();
    if (picked != null && mounted) {
      setState(() => _path.text = picked);
    }
  }

  void _turn(bool enabled) {
    final String path = _path.text.trim();
    _run(() =>
        widget.bridge.set(enabled: enabled, path: path.isEmpty ? null : path));
  }

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final NotesFolderStatus? status = _status;
    final bool enabled = status?.enabled ?? false;
    return Scaffold(
      appBar: AppBar(title: Text(l10n.notesFolder)),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          Text(l10n.keepYourProjectsAndMeetingsAs),
          const SizedBox(height: 12),
          Card(
            color: theme.colorScheme.errorContainer,
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Text(
                l10n.theFolderIsADecryptedCopy,
                style: TextStyle(color: theme.colorScheme.onErrorContainer),
              ),
            ),
          ),
          const SizedBox(height: 12),
          TextField(
            key: notesFolderPathFieldKey,
            controller: _path,
            enabled: !_busy,
            decoration: InputDecoration(
              labelText: l10n.folder,
              border: const OutlineInputBorder(),
              suffixIcon: IconButton(
                tooltip: l10n.chooseAFolder,
                icon: const Icon(Icons.folder_open_outlined),
                onPressed: _busy ? null : _choose,
              ),
            ),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: Text(l10n.keepTheNotesFolder),
            value: enabled,
            onChanged: _busy || status == null ? null : _turn,
          ),
          if (enabled)
            Align(
              alignment: Alignment.centerLeft,
              child: OutlinedButton.icon(
                icon: const Icon(Icons.sync),
                label: Text(l10n.syncNow),
                onPressed: _busy ? null : () => _run(widget.bridge.syncNow),
              ),
            ),
          if (_busy) const LinearProgressIndicator(),
          if (_error case final String error)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child:
                  Text(error, style: TextStyle(color: theme.colorScheme.error)),
            ),
          if (status?.lastRoundAt case final DateTime at)
            Padding(
              padding: const EdgeInsets.only(top: 8),
              child: Semantics(
                liveRegion: true,
                child: Text(
                  switch (status?.lastError) {
                    final String code => l10n.lastSyncAtFailed(
                        _clock(at), notesFolderErrorMessage(code)),
                    null => l10n.lastSyncAtFilesWrittenEdits(_clock(at),
                        status?.written ?? 0, status?.imported ?? 0),
                  },
                  style: theme.textTheme.bodySmall,
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// The local time as `HH:MM`.
String _clock(DateTime at) {
  final DateTime local = at.toLocal();
  return '${local.hour.toString().padLeft(2, '0')}:${local.minute.toString().padLeft(2, '0')}';
}
