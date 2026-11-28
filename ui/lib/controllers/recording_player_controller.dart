// Runs the kept-recording player of a meeting page (#197): play and pause, move anywhere, skip,
// change speed, and tell the transcript which passage is being heard so it can follow along.
//
// The kept recording is a WAV on the meeting's own timeline (silence fills the gaps between speech,
// see `core/bin/kept_recordings.rs`), so a passage's start time is its place in the recording. The
// core decrypts it to a private temporary file only when first played, and deletes that file when
// the page closes.

import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/recording_playback.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';

/// The speeds offered, as multiples of the normal speed.
const List<double> recordingPlaybackRates = <double>[
  0.75,
  1.0,
  1.25,
  1.5,
  1.75,
  2.0,
];

/// How far the skip buttons move, in milliseconds.
const int recordingSkipMs = 10000;

/// Returns the start of the passage being heard at `positionMs`: the last passage starting at or
/// before it, or `null` before the first one. `starts` must be sorted.
int? passageStartAt(List<int> starts, int positionMs) {
  int? current;
  for (final int start in starts) {
    if (start > positionMs) {
      break;
    }
    current = start;
  }
  return current;
}

/// The player's state and commands for one meeting's kept recording.
final class RecordingPlayerController extends ChangeNotifier {
  /// Creates the player for `meetingId`; `playback` defaults to `audioplayers`.
  ///
  /// Side effects: None until `check` or a command.
  RecordingPlayerController({
    required RecordingsBridge bridge,
    required this.meetingId,
    RecordingPlayback? playback,
  })  : _bridge = bridge,
        _playbackOverride = playback;

  final RecordingsBridge _bridge;
  final RecordingPlayback? _playbackOverride;

  /// Makes the playback of players created without one; tests replace it to play nothing.
  @visibleForTesting
  static RecordingPlayback Function() createPlayback = AudioPlayersPlayback.new;

  /// The meeting whose recording plays.
  final String meetingId;

  RecordingPlayback? _playback;
  StreamSubscription<Duration>? _positionSubscription;
  StreamSubscription<void>? _completionSubscription;
  bool _opened = false;
  bool _atEnd = false;
  bool _closed = false;

  /// The passage being heard, by its start in milliseconds; the transcript listens to this alone,
  /// so it rebuilds once per passage rather than at every position update.
  final ValueNotifier<int?> currentPassageStart = ValueNotifier<int?>(null);

  /// The transcript's passage starts, in milliseconds.
  List<int> get passageStarts => _passageStarts;
  List<int> _passageStarts = const <int>[];
  set passageStarts(List<int> starts) {
    _passageStarts = List<int>.of(starts)..sort();
    _updatePassage();
  }

  /// Whether the meeting has a kept recording.
  bool get available => _available;
  bool _available = false;

  /// Whether the recording is being decrypted for the first play.
  bool get preparing => _preparing;
  bool _preparing = false;

  /// Whether it is playing.
  bool get playing => _playing;
  bool _playing = false;

  /// Whether the last attempt to play failed.
  bool get failed => _failed;
  bool _failed = false;

  /// The position, in milliseconds.
  int get positionMs => _positionMs;
  int _positionMs = 0;

  /// The recording's length in milliseconds, or 0 until it is first opened.
  int get durationMs => _durationMs;
  int _durationMs = 0;

  /// The speed, as a multiple of the normal speed.
  double get rate => _rate;
  double _rate = 1.0;

  /// Whether the transcript keeps the passage being heard in view.
  bool get follow => _follow;
  bool _follow = true;

  /// Asks the core whether the meeting has a kept recording, without decrypting it.
  ///
  /// Side effects: One core command; notifies listeners. A failure leaves the player hidden.
  Future<void> check() async {
    try {
      _available = await _bridge.has(meetingId);
    } on Object {
      _available = false;
    }
    _notify();
  }

  /// Plays, or pauses when playing. At the end, plays from the start again.
  ///
  /// Side effects: The first play decrypts the recording to a temporary file.
  Future<void> toggle() async {
    if (_playing) {
      await _playback?.pause();
      _playing = false;
      _notify();
      return;
    }
    if (!await _ensureOpen()) {
      return;
    }
    if (_atEnd) {
      await _seek(0);
    }
    await _resume();
  }

  /// Plays from `positionMs`, such as a passage's start, and follows the transcript again.
  Future<void> playFrom(int positionMs) async {
    _follow = true;
    if (!await _ensureOpen()) {
      return;
    }
    await _seek(positionMs);
    if (!_playing) {
      await _resume();
    } else {
      _notify();
    }
  }

  /// Moves to `positionMs`, kept inside the recording, without changing play or pause.
  Future<void> seekTo(int positionMs) async {
    if (!await _ensureOpen()) {
      return;
    }
    await _seek(positionMs);
    _notify();
  }

  /// Moves by `deltaMs`, backwards when negative.
  Future<void> skip(int deltaMs) => seekTo(_positionMs + deltaMs);

  /// Plays at `rate` times the normal speed; chosen before the first play, it applies then.
  Future<void> setRate(double rate) async {
    _rate = rate;
    if (_opened) {
      await _playback?.setRate(rate);
    }
    _notify();
  }

  /// Stops keeping the passage being heard in view, as when the owner scrolls by hand.
  void stopFollowing() {
    if (_follow) {
      _follow = false;
      _notify();
    }
  }

  /// Keeps the passage being heard in view again.
  void followAgain() {
    if (!_follow) {
      _follow = true;
      _notify();
    }
  }

  /// Stops playing and lets go of the recording.
  ///
  /// Side effects: Frees the player and asks the core to delete the decrypted file.
  Future<void> close() async {
    if (_closed) {
      return;
    }
    _closed = true;
    // Started together, so a page being torn down does not wait on one before the next.
    final List<Future<void>> steps = <Future<void>>[
      if (_positionSubscription case final StreamSubscription<Duration> s)
        s.cancel(),
      if (_completionSubscription case final StreamSubscription<void> s)
        s.cancel(),
      if (_playback case final RecordingPlayback playback)
        _quietly(() async {
          await _quietly(playback.stop);
          await playback.dispose();
        }),
      // The core deletes the file at the next playback or start anyway, should this fail.
      if (_opened) _quietly(_bridge.release),
    ];
    await Future.wait(steps);
  }

  /// Runs `step`, ignoring a failure: closing goes on regardless.
  static Future<void> _quietly(Future<void> Function() step) async {
    try {
      await step();
    } on Object {
      // Nothing more to do.
    }
  }

  @override
  void dispose() {
    unawaited(close());
    currentPassageStart.dispose();
    super.dispose();
  }

  /// Decrypts and opens the recording the first time. Outputs: Whether it is open.
  Future<bool> _ensureOpen() async {
    if (_opened) {
      return true;
    }
    if (_preparing || _closed) {
      return false;
    }
    _preparing = true;
    _failed = false;
    _notify();
    try {
      final PreparedRecording recording = await _bridge.prepare(meetingId);
      final RecordingPlayback playback =
          _playback ??= _playbackOverride ?? createPlayback();
      _positionSubscription ??= playback.positions.listen(_onPosition);
      _completionSubscription ??= playback.completions.listen((_) {
        _playing = false;
        _atEnd = true;
        _positionMs = _durationMs;
        _notify();
      });
      await playback.open(recording.path);
      if (_rate != 1.0) {
        await playback.setRate(_rate);
      }
      _durationMs = recording.durationMs;
      _opened = true;
      _updatePassage();
      return true;
    } on Object {
      _failed = true;
      return false;
    } finally {
      _preparing = false;
      _notify();
    }
  }

  Future<void> _resume() async {
    await _playback?.resume();
    _playing = true;
    _atEnd = false;
    _notify();
  }

  Future<void> _seek(int positionMs) async {
    final int clamped =
        _durationMs > 0 ? positionMs.clamp(0, _durationMs) : positionMs;
    final int target = clamped < 0 ? 0 : clamped;
    await _playback?.seek(Duration(milliseconds: target));
    _atEnd = false;
    _positionMs = target;
    _updatePassage();
  }

  void _onPosition(Duration position) {
    _positionMs = position.inMilliseconds;
    _updatePassage();
    _notify();
  }

  /// Marks a passage only once the recording is open, so nothing is marked before playing.
  void _updatePassage() {
    if (!_closed) {
      currentPassageStart.value =
          _opened ? passageStartAt(_passageStarts, _positionMs) : null;
    }
  }

  void _notify() {
    if (!_closed) {
      notifyListeners();
    }
  }
}
