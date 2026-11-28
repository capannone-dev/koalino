// Kept meeting recordings in the app (#140): the Settings section (switch with its privacy and
// space warning, what is kept, Wipe all recordings). The meeting page plays them with
// `recording_player_bar.dart` (#197).
//
// The widgets stay dumb: every decision and file lives in the core, reached through
// `RecordingsBridge`.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Key of the Keep meeting recordings switch.
const Key keepRecordingsSwitchKey = ValueKey<String>('keep-recordings');

/// Key of the Wipe all recordings button.
const Key wipeRecordingsKey = ValueKey<String>('wipe-recordings');

/// Words a size in bytes as MB with one decimal.
String recordingsSize(int bytes) =>
    (bytes / (1024 * 1024)).toStringAsFixed(bytes < 10 * 1024 * 1024 ? 1 : 0);

/// The Settings section: the switch, what is kept and Wipe all recordings.
final class RecordingsSection extends StatefulWidget {
  /// Creates the section over `bridge`.
  ///
  /// Side effects: Reads the status when shown.
  const RecordingsSection({required this.bridge, super.key});

  final RecordingsBridge bridge;

  @override
  State<RecordingsSection> createState() => _RecordingsSectionState();
}

final class _RecordingsSectionState extends State<RecordingsSection> {
  RecordingsStatus? _status;
  bool _busy = false;
  String? _message;

  @override
  void initState() {
    super.initState();
    unawaited(_load());
  }

  Future<void> _load() async {
    try {
      final RecordingsStatus status = await widget.bridge.status();
      if (mounted) setState(() => _status = status);
    } on LlmException {
      if (mounted) setState(() => _message = l10n.recordingsUnavailable);
    }
  }

  /// Turns keeping on after the owner reads the warning, or off at once.
  Future<void> _toggle(bool keep) async {
    if (keep) {
      final bool? confirmed = await showDialog<bool>(
        context: context,
        builder: (BuildContext context) => AlertDialog(
          title: Text(l10n.keepMeetingRecordingsQuestion),
          content:
              SingleChildScrollView(child: Text(l10n.keepRecordingsWarning)),
          actions: <Widget>[
            TextButton(
              onPressed: () => Navigator.of(context).pop(false),
              child: Text(l10n.cancel),
            ),
            FilledButton(
              key: const ValueKey<String>('confirm-keep-recordings'),
              onPressed: () => Navigator.of(context).pop(true),
              child: Text(l10n.keepRecordings),
            ),
          ],
        ),
      );
      if (confirmed != true) return;
    }
    await _run(() async {
      final RecordingsStatus status = await widget.bridge.setKeep(keep);
      setState(() => _status = status);
    });
  }

  Future<void> _wipe() async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext context) => AlertDialog(
        title: Text(l10n.wipeAllRecordingsQuestion),
        content: Text(l10n.wipeAllRecordingsExplanation),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            key: const ValueKey<String>('confirm-wipe-recordings'),
            style: FilledButton.styleFrom(
                backgroundColor: Theme.of(context).colorScheme.error,
                foregroundColor: Theme.of(context).colorScheme.onError),
            onPressed: () => Navigator.of(context).pop(true),
            child: Text(l10n.wipeAllRecordings),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    await _run(() async {
      final int deleted = await widget.bridge.wipe();
      final RecordingsStatus status = await widget.bridge.status();
      setState(() {
        _status = status;
        _message = l10n.recordingsWiped(deleted);
      });
    });
  }

  Future<void> _run(Future<void> Function() work) async {
    setState(() {
      _busy = true;
      _message = null;
    });
    try {
      await work();
    } on LlmException {
      if (mounted) setState(() => _message = l10n.recordingsUnavailable);
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final RecordingsStatus? status = _status;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: <Widget>[
        SwitchListTile(
          key: keepRecordingsSwitchKey,
          secondary: const Icon(Icons.graphic_eq),
          title: Text(l10n.keepMeetingRecordings),
          subtitle: Text(l10n.keepMeetingRecordingsSubtitle),
          isThreeLine: true,
          value: status?.keep ?? false,
          onChanged: status == null || _busy ? null : _toggle,
        ),
        ListTile(
          leading: const SizedBox(width: 24),
          title: Text(status == null
              ? l10n.loading
              : l10n.recordingsKept(
                  status.count, recordingsSize(status.bytes))),
          subtitle: _message == null
              ? null
              : Text(_message!,
                  key: const ValueKey<String>('recordings-message')),
          trailing: OutlinedButton.icon(
            key: wipeRecordingsKey,
            icon: const Icon(Icons.delete_forever_outlined),
            label: Text(l10n.wipeAllRecordings),
            onPressed:
                status == null || status.count == 0 || _busy ? null : _wipe,
          ),
        ),
      ],
    );
  }
}
