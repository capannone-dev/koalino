// Kept meeting recordings in the app (#140): the Settings section (switch with its privacy and
// space warning, what is kept, Wipe all recordings) and the meeting page's Play recording button.
//
// The widgets stay dumb: every decision and file lives in the core, reached through
// `RecordingsBridge`. Playback uses `audioplayers` on the one decrypted file the core prepares,
// which is released when playback stops or the page closes.

import 'dart:async';

import 'package:audioplayers/audioplayers.dart';
import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Key of the Keep meeting recordings switch.
const Key keepRecordingsSwitchKey = ValueKey<String>('keep-recordings');

/// Key of the Wipe all recordings button.
const Key wipeRecordingsKey = ValueKey<String>('wipe-recordings');

/// Key of the meeting page's Play recording button.
const Key playRecordingKey = ValueKey<String>('play-recording');

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

/// The meeting page's Play recording button, shown only when the meeting has a kept recording.
final class PlayRecordingButton extends StatefulWidget {
  /// Creates the button; `player` is replaced in tests.
  ///
  /// Side effects: Asks the core whether a recording is kept when shown.
  const PlayRecordingButton(
      {required this.bridge, required this.meetingId, this.player, super.key});

  final RecordingsBridge bridge;
  final String meetingId;
  final AudioPlayer? player;

  @override
  State<PlayRecordingButton> createState() => _PlayRecordingButtonState();
}

final class _PlayRecordingButtonState extends State<PlayRecordingButton> {
  bool _available = false;
  bool _playing = false;
  bool _preparing = false;
  AudioPlayer? _player;
  StreamSubscription<void>? _finished;

  @override
  void initState() {
    super.initState();
    unawaited(_check());
  }

  Future<void> _check() async {
    try {
      final bool available = await widget.bridge.has(widget.meetingId);
      if (mounted) setState(() => _available = available);
    } on LlmException {
      // No core or no recording: the button stays hidden.
    }
  }

  AudioPlayer _ensurePlayer() {
    final AudioPlayer player = _player ??= widget.player ?? AudioPlayer();
    _finished ??= player.onPlayerComplete.listen((_) => unawaited(_stop()));
    return player;
  }

  Future<void> _play() async {
    setState(() => _preparing = true);
    try {
      final PreparedRecording recording =
          await widget.bridge.prepare(widget.meetingId);
      if (!mounted) return;
      await _ensurePlayer().play(DeviceFileSource(recording.path));
      if (mounted) setState(() => _playing = true);
    } on LlmException {
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
            SnackBar(content: Text(l10n.recordingCouldNotBePlayed)));
      }
    } finally {
      if (mounted) setState(() => _preparing = false);
    }
  }

  Future<void> _stop() async {
    await _player?.stop();
    if (mounted) setState(() => _playing = false);
    try {
      await widget.bridge.release();
    } on LlmException {
      // The core deletes the file at the next playback or start anyway.
    }
  }

  @override
  void dispose() {
    unawaited(_finished?.cancel());
    if (_playing) {
      unawaited(_player?.stop());
      unawaited(widget.bridge.release().catchError((Object _) {}));
    }
    if (widget.player == null) unawaited(_player?.dispose());
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    if (!_available) return const SizedBox.shrink();
    return IconButton(
      key: playRecordingKey,
      tooltip: _playing ? l10n.stopRecordingPlayback : l10n.playRecording,
      icon: _preparing
          ? const SizedBox.square(
              dimension: 20, child: CircularProgressIndicator(strokeWidth: 2))
          : Icon(_playing
              ? Icons.stop_circle_outlined
              : Icons.play_circle_outline),
      onPressed: _preparing ? null : (_playing ? _stop : _play),
    );
  }
}
