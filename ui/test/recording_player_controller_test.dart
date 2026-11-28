// Specifies the kept-recording player (#197): it decrypts the recording only when first played,
// moves anywhere in it, changes speed, follows the passage being heard, and releases the file.
import 'dart:async';
import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/recording_player_controller.dart';
import 'package:note_taker_ui/infrastructure/recording_playback.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';

/// Answers the core's recordings commands and records them.
final class _FakeCore {
  final List<String> commands = <String>[];
  bool available = true;

  Future<String> call(String function, List<String?> arguments) async {
    final Map<String, Object?> request =
        jsonDecode(arguments.single!) as Map<String, Object?>;
    commands.add(request['command']! as String);
    return jsonEncode(switch (request['command']) {
      'has' => <String, Object?>{'ok': true, 'available': available},
      'prepare' => <String, Object?>{
          'ok': true,
          'path': '/private/recording.wav',
          'duration_ms': 600000,
        },
      'release' => <String, Object?>{'ok': true},
      _ => <String, Object?>{'ok': false, 'error_code': 'invalid_request'},
    });
  }
}

/// Plays nothing; records what it was asked and reports positions on demand.
final class _FakePlayback implements RecordingPlayback {
  final List<String> calls = <String>[];
  final StreamController<Duration> _positions =
      StreamController<Duration>.broadcast();
  final StreamController<void> _completions =
      StreamController<void>.broadcast();

  @override
  Stream<Duration> get positions => _positions.stream;

  @override
  Stream<void> get completions => _completions.stream;

  void reach(int ms) => _positions.add(Duration(milliseconds: ms));

  void end() => _completions.add(null);

  @override
  Future<void> open(String path) async => calls.add('open $path');

  @override
  Future<void> resume() async => calls.add('resume');

  @override
  Future<void> pause() async => calls.add('pause');

  @override
  Future<void> stop() async => calls.add('stop');

  @override
  Future<void> seek(Duration position) async =>
      calls.add('seek ${position.inMilliseconds}');

  @override
  Future<void> setRate(double rate) async => calls.add('rate $rate');

  @override
  Future<void> dispose() async {
    calls.add('dispose');
    await _positions.close();
    await _completions.close();
  }
}

void main() {
  late _FakeCore core;
  late _FakePlayback playback;
  late RecordingPlayerController player;

  setUp(() {
    core = _FakeCore();
    playback = _FakePlayback();
    player = RecordingPlayerController(
      bridge: RecordingsBridge(call: core.call),
      meetingId: 'm-1',
      playback: playback,
    );
  });

  test('a meeting without a kept recording has no player', () async {
    core.available = false;
    await player.check();
    expect(player.available, isFalse);
    expect(core.commands, <String>['has']);
  });

  test('the recording is decrypted only when first played', () async {
    await player.check();
    expect(player.available, isTrue);
    expect(core.commands, <String>['has']);

    await player.toggle();
    expect(core.commands, <String>['has', 'prepare']);
    expect(playback.calls, <String>['open /private/recording.wav', 'resume']);
    expect(player.playing, isTrue);
    expect(player.durationMs, 600000);

    await player.toggle();
    expect(player.playing, isFalse);
    expect(playback.calls.last, 'pause');
    await player.toggle();
    expect(core.commands.where((String c) => c == 'prepare'), hasLength(1),
        reason: 'a pause keeps the file');
  });

  test('it moves anywhere, skips, and stays inside the recording', () async {
    await player.check();
    await player.toggle();
    await player.seekTo(120000);
    expect(playback.calls.last, 'seek 120000');
    expect(player.positionMs, 120000);

    await player.skip(-10000);
    expect(playback.calls.last, 'seek 110000');
    await player.seekTo(0);
    await player.skip(-10000);
    expect(playback.calls.last, 'seek 0');
    await player.skip(700000);
    expect(playback.calls.last, 'seek 600000');
  });

  test('the speed changes, also before the first play', () async {
    await player.check();
    await player.setRate(1.5);
    expect(player.rate, 1.5);
    await player.toggle();
    expect(playback.calls, contains('rate 1.5'),
        reason: 'the speed chosen before opening applies once it opens');
    await player.setRate(0.75);
    expect(playback.calls.last, 'rate 0.75');
  });

  test('it reports the passage being heard', () async {
    player.passageStarts = <int>[0, 5000, 12000, 30000];
    await player.check();
    expect(player.currentPassageStart.value, isNull,
        reason: 'nothing is marked before playing');
    await player.toggle();
    expect(player.currentPassageStart.value, 0);
    playback.reach(13000);
    await Future<void>.delayed(Duration.zero);
    expect(player.positionMs, 13000);
    expect(player.currentPassageStart.value, 12000);

    playback.reach(4000);
    await Future<void>.delayed(Duration.zero);
    expect(player.currentPassageStart.value, 0);
  });

  test('playing from a passage opens the recording and starts there', () async {
    await player.check();
    await player.playFrom(30000);
    expect(playback.calls,
        <String>['open /private/recording.wav', 'seek 30000', 'resume']);
    expect(player.playing, isTrue);
    expect(player.follow, isTrue);
  });

  test('scrolling by hand stops following until asked again', () async {
    await player.check();
    await player.toggle();
    player.stopFollowing();
    expect(player.follow, isFalse);
    await player.playFrom(5000);
    expect(player.follow, isTrue, reason: 'playing a passage follows again');
    player.stopFollowing();
    player.followAgain();
    expect(player.follow, isTrue);
  });

  test('at the end it stops, and the next play starts over', () async {
    await player.check();
    await player.toggle();
    playback.end();
    await Future<void>.delayed(Duration.zero);
    expect(player.playing, isFalse);
    await player.toggle();
    expect(playback.calls.sublist(playback.calls.length - 2),
        <String>['seek 0', 'resume']);
  });

  test('closing the page releases the decrypted file', () async {
    await player.check();
    await player.toggle();
    await player.close();
    expect(core.commands.last, 'release');
    expect(playback.calls, containsAll(<String>['stop', 'dispose']));
  });

  test('a recording that cannot be decrypted reports it', () async {
    await player.check();
    final RecordingPlayerController broken = RecordingPlayerController(
      bridge: RecordingsBridge(
          call: (String function, List<String?> arguments) async =>
              jsonEncode(<String, Object?>{
                'ok': false,
                'error_code': 'not_found',
              })),
      meetingId: 'm-1',
      playback: _FakePlayback(),
    );
    await broken.toggle();
    expect(broken.playing, isFalse);
    expect(broken.failed, isTrue);
  });
}
