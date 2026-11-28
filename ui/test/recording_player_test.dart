// Specifies the meeting page's recording player (#197): it shows only for a kept recording, plays
// from a passage, marks the passage being heard, changes speed, and releases the file on leaving.
import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/controllers/recording_player_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/recording_playback.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/recording_player_bar.dart';

import 'fake_meeting_bridge.dart';

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
          'duration_ms': 65000,
        },
      _ => <String, Object?>{'ok': true},
    });
  }
}

/// Plays nothing and records what it was asked.
final class _FakePlayback implements RecordingPlayback {
  final List<String> calls = <String>[];
  final StreamController<Duration> positionEvents =
      StreamController<Duration>.broadcast();

  @override
  Stream<Duration> get positions => positionEvents.stream;

  @override
  Stream<void> get completions => const Stream<void>.empty();

  @override
  Future<void> open(String path) async => calls.add('open');

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
  Future<void> dispose() async => calls.add('dispose');
}

void main() {
  late _FakeCore core;
  late _FakePlayback playback;

  setUp(() {
    core = _FakeCore();
    playback = _FakePlayback();
    RecordingPlayerController.createPlayback = () => playback;
  });

  tearDown(() =>
      RecordingPlayerController.createPlayback = AudioPlayersPlayback.new);

  Future<void> pumpMeeting(WidgetTester tester) async {
    final MeetingListController controller = MeetingListController(
        meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
      'meeting-titled': titledMeetingDetail,
    }));
    await tester.pumpWidget(MaterialApp(
      home: MeetingDetailScreen(
        controller: controller,
        meetingId: 'meeting-titled',
        recordingsBridge: RecordingsBridge(call: core.call),
      ),
    ));
    await tester.pumpAndSettle();
  }

  testWidgets('a meeting without a kept recording shows no player',
      (WidgetTester tester) async {
    core.available = false;
    await pumpMeeting(tester);
    expect(find.byKey(recordingPlayerBarKey), findsNothing);
    expect(find.byTooltip('Play from here'), findsNothing);
  });

  testWidgets('a passage plays from its start and is marked while heard',
      (WidgetTester tester) async {
    await pumpMeeting(tester);
    expect(find.byKey(recordingPlayerBarKey), findsOneWidget);
    expect(find.text('0:00 / 1:05'), findsOneWidget,
        reason: 'the meeting length shows before the recording opens');
    expect(core.commands, <String>['has'],
        reason: 'nothing is decrypted before playing');

    await tester.tap(find.byKey(const ValueKey<String>('play-from-61000')));
    await tester.pumpAndSettle();
    expect(playback.calls, <String>['open', 'seek 61000', 'resume']);
    expect(
        find.descendant(
            of: find.byKey(heardPassageKey),
            matching: find.text('Unattributed · 1:01')),
        findsOneWidget);

    playback.positionEvents.add(const Duration(milliseconds: 2000));
    await tester.pumpAndSettle();
    expect(
        find.descendant(
            of: find.byKey(heardPassageKey),
            matching: find.text('Alice · 0:00')),
        findsOneWidget);
    expect(find.text('0:02 / 1:05'), findsOneWidget);

    await tester.tap(find.byKey(recordingPlayPauseKey));
    await tester.pumpAndSettle();
    expect(playback.calls.last, 'pause');
  });

  testWidgets('the speed changes from the bar', (WidgetTester tester) async {
    await pumpMeeting(tester);
    await tester.tap(find.byKey(recordingSpeedKey));
    await tester.pumpAndSettle();
    await tester.tap(find.text('1.5×').last);
    await tester.pumpAndSettle();
    expect(find.text('1.5×'), findsOneWidget);

    await tester.tap(find.byKey(recordingPlayPauseKey));
    await tester.pumpAndSettle();
    expect(playback.calls, <String>['open', 'rate 1.5', 'resume']);
  });

  testWidgets('leaving the page releases the decrypted recording',
      (WidgetTester tester) async {
    await pumpMeeting(tester);
    await tester.tap(find.byKey(recordingPlayPauseKey));
    await tester.pumpAndSettle();
    await tester.pumpWidget(const MaterialApp(home: SizedBox()));
    await tester.pumpAndSettle();
    expect(playback.calls, containsAll(<String>['stop', 'dispose']));
    expect(core.commands.last, 'release');
  });

  test('speeds read without trailing zeros', () {
    expect(formatPlaybackRate(1.0), '1×');
    expect(formatPlaybackRate(1.25), '1.25×');
    expect(formatPlaybackRate(0.75), '0.75×');
    expect(formatPlaybackRate(2.0), '2×');
  });
}
