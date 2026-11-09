// The notes folder page (#98), in Settings on desktops: turns on a folder of plain files that
// mirrors projects and meetings, warns that it is a decrypted copy, and shows the last round.

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/notes_folder_bridge.dart';

/// Finds the folder field in tests.
const Key notesFolderPathFieldKey = ValueKey<String>('notes-folder-path');

/// Picks a folder, or null when cancelled; replaced in tests.
typedef FolderPicker = Future<String?> Function();

/// Opens the system's folder dialog.
Future<String?> pickFolderWithDialog() =>
    FilePicker.platform.getDirectoryPath(dialogTitle: 'Notes folder');

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
      appBar: AppBar(title: const Text('Notes folder')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: <Widget>[
          const Text(
              'Keep your projects and meetings as plain files in a folder, to read and '
              'edit them with any editor. Edits to notes.md, summary.md and project files '
              'come back into the app within half a minute, and files you drop into a '
              'project folder or a meeting\'s attachments folder are added. '
              'transcript.md is rewritten by the app: correct the transcript in the app.'),
          const SizedBox(height: 12),
          Card(
            color: theme.colorScheme.errorContainer,
            child: Padding(
              padding: const EdgeInsets.all(12),
              child: Text(
                'The folder is a decrypted copy of your notes. Anyone who can read it can '
                'read them, and backup or sync tools will copy it. Keep it on an encrypted '
                'disk, inside your own user folder.',
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
              labelText: 'Folder',
              border: const OutlineInputBorder(),
              suffixIcon: IconButton(
                tooltip: 'Choose a folder',
                icon: const Icon(Icons.folder_open_outlined),
                onPressed: _busy ? null : _choose,
              ),
            ),
          ),
          SwitchListTile(
            contentPadding: EdgeInsets.zero,
            title: const Text('Keep the notes folder'),
            value: enabled,
            onChanged: _busy || status == null ? null : _turn,
          ),
          if (enabled)
            Align(
              alignment: Alignment.centerLeft,
              child: OutlinedButton.icon(
                icon: const Icon(Icons.sync),
                label: const Text('Sync now'),
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
                    final String code =>
                      'Last sync at ${_clock(at)} failed: ${notesFolderErrorMessage(code)}',
                    null =>
                      'Last sync at ${_clock(at)}: ${status?.written ?? 0} files written, '
                          '${status?.imported ?? 0} edits and files taken in.',
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
