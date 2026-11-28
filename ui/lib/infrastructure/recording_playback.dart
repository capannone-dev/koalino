// Plays one local audio file for the kept-recording player (#197), behind a small interface so
// the player's logic is tested without audio. The app's implementation uses `audioplayers`.

import 'package:audioplayers/audioplayers.dart';

/// Plays one local file: open, play, pause, move, and change speed.
abstract interface class RecordingPlayback {
  /// The playing position, reported a few times a second while playing.
  Stream<Duration> get positions;

  /// An event each time playback reaches the end.
  Stream<void> get completions;

  /// Loads the file at `path`, paused at its start. Side effects: Opens the file.
  Future<void> open(String path);

  /// Plays from the current position.
  Future<void> resume();

  /// Pauses, keeping the position.
  Future<void> pause();

  /// Stops and lets go of the file.
  Future<void> stop();

  /// Moves to `position`.
  Future<void> seek(Duration position);

  /// Plays at `rate` times the normal speed, keeping the pitch where the platform can.
  Future<void> setRate(double rate);

  /// Frees the player for good.
  Future<void> dispose();
}

/// `RecordingPlayback` over `audioplayers` (ExoPlayer on Android, GStreamer on Linux, Media
/// Foundation on Windows, AVPlayer on Apple systems).
final class AudioPlayersPlayback implements RecordingPlayback {
  /// Creates a player; `player` is injectable for tests.
  AudioPlayersPlayback({AudioPlayer? player})
      : _player = player ?? AudioPlayer();

  final AudioPlayer _player;

  @override
  Stream<Duration> get positions => _player.onPositionChanged;

  @override
  Stream<void> get completions => _player.onPlayerComplete;

  @override
  Future<void> open(String path) async {
    // The default releases the file at the end, so a later Play would find nothing loaded.
    await _player.setReleaseMode(ReleaseMode.stop);
    await _player.setSource(DeviceFileSource(path));
  }

  @override
  Future<void> resume() => _player.resume();

  @override
  Future<void> pause() => _player.pause();

  @override
  Future<void> stop() => _player.stop();

  @override
  Future<void> seek(Duration position) => _player.seek(position);

  @override
  Future<void> setRate(double rate) => _player.setPlaybackRate(rate);

  @override
  Future<void> dispose() => _player.dispose();
}
