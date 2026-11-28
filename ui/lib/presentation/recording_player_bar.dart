// The meeting page's player for a kept recording (#197): a bar under the transcript with the
// position slider, back and forward 10 seconds, play and pause, the speed, and a button to follow
// the transcript again after scrolling away. The logic lives in `RecordingPlayerController`.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/recording_player_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Key of the player bar.
const Key recordingPlayerBarKey = ValueKey<String>('recording-player');

/// Key of the play and pause button.
const Key recordingPlayPauseKey = ValueKey<String>('recording-play-pause');

/// Key of the speed menu.
const Key recordingSpeedKey = ValueKey<String>('recording-speed');

/// Key of the position slider.
const Key recordingPositionKey = ValueKey<String>('recording-position');

/// Key of the button that follows the transcript again.
const Key recordingFollowKey = ValueKey<String>('recording-follow');

/// Words a speed as `1.25×`, without a trailing zero.
String formatPlaybackRate(double rate) {
  final String text = rate.toStringAsFixed(2);
  return '${text.replaceFirst(RegExp(r'\.?0+$'), '')}×';
}

/// The player bar; nothing while the meeting has no kept recording.
final class RecordingPlayerBar extends StatefulWidget {
  /// Creates the bar over `player`; `fallbackDurationMs` is the meeting's length, shown until the
  /// recording is first opened and its own length is known.
  const RecordingPlayerBar({
    required this.player,
    this.fallbackDurationMs = 0,
    super.key,
  });

  final RecordingPlayerController player;
  final int fallbackDurationMs;

  @override
  State<RecordingPlayerBar> createState() => _RecordingPlayerBarState();
}

final class _RecordingPlayerBarState extends State<RecordingPlayerBar> {
  /// Where the slider is being dragged; the recording moves there once it is let go.
  double? _draggedMs;

  @override
  Widget build(BuildContext context) {
    final RecordingPlayerController player = widget.player;
    final int fallbackDurationMs = widget.fallbackDurationMs;
    return ListenableBuilder(
      listenable: player,
      builder: (BuildContext context, Widget? child) {
        if (!player.available) {
          return const SizedBox.shrink();
        }
        final ThemeData theme = Theme.of(context);
        final int duration =
            player.durationMs > 0 ? player.durationMs : fallbackDurationMs;
        final int position = player.positionMs.clamp(0, duration);
        return Material(
          key: recordingPlayerBarKey,
          color: theme.colorScheme.surfaceContainer,
          child: SafeArea(
            top: false,
            child: Semantics(
              container: true,
              label: l10n.recordingPlayer,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(8, 4, 8, 4),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: <Widget>[
                    Slider(
                      key: recordingPositionKey,
                      value: duration > 0
                          ? (_draggedMs ?? position.toDouble())
                              .clamp(0, duration.toDouble())
                          : 0,
                      max: duration > 0 ? duration.toDouble() : 1,
                      label: formatMeetingDuration(
                          (_draggedMs ?? position.toDouble()).round()),
                      onChanged: duration > 0
                          ? (double value) => setState(() => _draggedMs = value)
                          : null,
                      onChangeEnd: (double value) {
                        setState(() => _draggedMs = null);
                        player.seekTo(value.round());
                      },
                    ),
                    Row(
                      children: <Widget>[
                        Padding(
                          padding: const EdgeInsetsDirectional.only(start: 8),
                          child: Text(
                            player.preparing
                                ? l10n.openingTheRecording
                                : player.failed
                                    ? l10n.recordingCouldNotBePlayed
                                    : '${formatMeetingDuration(position)} / '
                                        '${formatMeetingDuration(duration)}',
                            style: theme.textTheme.labelMedium,
                          ),
                        ),
                        const Spacer(),
                        IconButton(
                          tooltip: l10n.skipBack10Seconds,
                          icon: const Icon(Icons.replay_10),
                          onPressed: player.preparing
                              ? null
                              : () => player.skip(-recordingSkipMs),
                        ),
                        IconButton.filled(
                          key: recordingPlayPauseKey,
                          tooltip: player.playing ? l10n.pause : l10n.play,
                          icon: player.preparing
                              ? const SizedBox.square(
                                  dimension: 20,
                                  child:
                                      CircularProgressIndicator(strokeWidth: 2))
                              : Icon(player.playing
                                  ? Icons.pause
                                  : Icons.play_arrow),
                          onPressed: player.preparing ? null : player.toggle,
                        ),
                        IconButton(
                          tooltip: l10n.skipForward10Seconds,
                          icon: const Icon(Icons.forward_10),
                          onPressed: player.preparing
                              ? null
                              : () => player.skip(recordingSkipMs),
                        ),
                        const Spacer(),
                        if (!player.follow && player.playing)
                          IconButton(
                            key: recordingFollowKey,
                            tooltip: l10n.followTranscript,
                            icon: const Icon(Icons.vertical_align_center),
                            onPressed: player.followAgain,
                          ),
                        PopupMenuButton<double>(
                          key: recordingSpeedKey,
                          tooltip: l10n.playbackSpeed,
                          initialValue: player.rate,
                          onSelected: player.setRate,
                          itemBuilder: (BuildContext context) =>
                              <PopupMenuEntry<double>>[
                            for (final double rate in recordingPlaybackRates)
                              PopupMenuItem<double>(
                                value: rate,
                                child: Text(formatPlaybackRate(rate)),
                              ),
                          ],
                          child: Padding(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 12, vertical: 8),
                            child: Text(formatPlaybackRate(player.rate),
                                style: theme.textTheme.labelLarge),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}
