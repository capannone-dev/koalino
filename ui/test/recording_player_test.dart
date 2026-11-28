// Specifies the meeting page's recording player (#197): it shows only for a kept recording, plays
// from a passage, marks the passage being heard, changes speed, and releases the file on leaving.
// It also shares the recording (Android) or saves it (desktop) (#199).
import 'dart:async';
import 'dart:convert';
import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_export_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/export_bridge.dart';
import 'package:note_taker_ui/controllers/recording_player_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/recording_playback.dart';
import 'package:note_taker_ui/infrastructure/recordings_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/recording_player_bar.dart';

import 'fake_log_bridge.dart';
import 'fake_meeting_bridge.dart';

/// Answers the core's recordings commands and records them.
final class _FakeCore {
  final List<String> commands = <String>[];
  bool available = true;

  /// The `path` of each `export`, which the fake writes as the core would.
  final List<String> exportedTo = <String>[];

  /// Whether `export` fails, as when the recording was wiped meanwhile.
  bool exportFails = false;

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
      'export' when exportFails => <String, Object?>{
          'ok': false,
          'error_code': 'not_found',
          'error': 'no recording is kept for this meeting',
        },
      'export' => () {
          final String path = request['path']! as String;
          exportedTo.add(path);
          File(path).writeAsBytesSync(<int>[1, 2, 3]);
          return <String, Object?>{
            'ok': true,
            'path': path,
            'duration_ms': 65000
          };
        }(),
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

  Future<void> pumpMeeting(WidgetTester tester,
      {MeetingExportController? exportController}) async {
    final MeetingListController controller = MeetingListController(
        meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
      'meeting-titled': titledMeetingDetail,
    }));
    await controller.selectMeeting('meeting-titled');
    await tester.pumpWidget(MaterialApp(
      home: MeetingDetailScreen(
        controller: controller,
        meetingId: 'meeting-titled',
        recordingsBridge: RecordingsBridge(call: core.call),
        exportController: exportController,
      ),
    ));
    await tester.pumpAndSettle();
  }

  /// An export controller over `saver` and, on Android, `sharer`.
  MeetingExportController exports(ExportSaver saver, {ExportSharer? sharer}) =>
      MeetingExportController(
          exportBridge: _NoMeetingExports(),
          exportSaver: saver,
          exportSharer: sharer);

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

  testWidgets('Space plays and pauses, J and L skip 10 seconds (#215)',
      (WidgetTester tester) async {
    await pumpMeeting(tester);

    await tester.sendKeyEvent(LogicalKeyboardKey.space);
    await tester.pumpAndSettle();
    expect(playback.calls, contains('resume'));
    await simulateKeyDownEvent(LogicalKeyboardKey.keyL, character: 'l');
    await simulateKeyUpEvent(LogicalKeyboardKey.keyL);
    await tester.pumpAndSettle();
    expect(playback.calls.last, startsWith('seek '));
    final int forward = int.parse(playback.calls.last.split(' ').last);
    await simulateKeyDownEvent(LogicalKeyboardKey.keyJ, character: 'j');
    await simulateKeyUpEvent(LogicalKeyboardKey.keyJ);
    await tester.pumpAndSettle();
    expect(int.parse(playback.calls.last.split(' ').last), forward - 10000);
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

  testWidgets('on Android the recording is shared from the bar (#199)',
      (WidgetTester tester) async {
    final Directory folder =
        Directory.systemTemp.createTempSync('ui-share-recording');
    addTearDown(() => folder.deleteSync(recursive: true));
    File('${folder.path}/old export.pdf').writeAsStringSync('left over');
    final _RecordingSharer sharer = _RecordingSharer(folder);
    await pumpMeeting(tester,
        exportController: exports(FakeDesktopSaver(), sharer: sharer));

    await tester.runAsync(() async {
      await tester.tap(find.byKey(recordingExportKey));
      await Future<void>.delayed(const Duration(milliseconds: 100));
    });
    await tester.pumpAndSettle();

    expect(find.byTooltip('Share the recording'), findsOneWidget);
    expect(core.exportedTo, <String>['${folder.path}/Project Sync.wav']);
    expect(sharer.shared, <String>['${folder.path}/Project Sync.wav']);
    expect(File('${folder.path}/old export.pdf').existsSync(), isFalse,
        reason: 'the share folder is emptied first');
    expect(core.commands, isNot(contains('prepare')),
        reason: 'sharing does not open the player');
  });

  testWidgets('on desktop the recording is saved where the owner chooses',
      (WidgetTester tester) async {
    final Directory folder =
        Directory.systemTemp.createTempSync('ui-save-recording');
    addTearDown(() => folder.deleteSync(recursive: true));
    final FakeDesktopSaver saver =
        FakeDesktopSaver(chosenPath: '${folder.path}/chosen.wav');
    final FakeDesktopSaver meetingExports = FakeDesktopSaver();
    await pumpMeeting(tester,
        exportController: MeetingExportController(
            exportBridge: _NoMeetingExports(),
            exportSaver: meetingExports,
            recordingSaver: saver));

    await tester.runAsync(() async {
      await tester.tap(find.byKey(recordingExportKey));
      await Future<void>.delayed(const Duration(milliseconds: 100));
    });
    await tester.pumpAndSettle();

    expect(find.byTooltip('Save the recording'), findsOneWidget);
    expect(saver.suggestedNames, <String>['Project Sync.wav']);
    expect(meetingExports.suggestedNames, isEmpty,
        reason: 'the recording has its own save dialog');
    expect(core.exportedTo, <String>['${folder.path}/chosen.wav']);
    expect(find.text('Exported to ${folder.path}/chosen.wav'), findsOneWidget);
  });

  testWidgets('without a save dialog the recording goes to Downloads',
      (WidgetTester tester) async {
    final Directory downloads =
        Directory.systemTemp.createTempSync('ui-downloads');
    addTearDown(() => downloads.deleteSync(recursive: true));
    File('${downloads.path}/Project Sync.wav').writeAsStringSync('older');
    await pumpMeeting(tester,
        exportController: MeetingExportController(
            exportBridge: _NoMeetingExports(),
            exportSaver: _NoDialogSaver(),
            downloadsDirectory: () async => downloads.path));

    await tester.runAsync(() async {
      await tester.tap(find.byKey(recordingExportKey));
      await Future<void>.delayed(const Duration(milliseconds: 100));
    });
    await tester.pumpAndSettle();

    expect(core.exportedTo, <String>['${downloads.path}/Project Sync (2).wav'],
        reason: 'an existing file is not replaced');
    expect(find.text('Exported to ${downloads.path}/Project Sync (2).wav'),
        findsOneWidget);
  });

  testWidgets('a recording that cannot be written says so',
      (WidgetTester tester) async {
    core.exportFails = true;
    await pumpMeeting(tester,
        exportController: exports(FakeDesktopSaver(chosenPath: '/x/y.wav')));
    await tester.runAsync(() async {
      await tester.tap(find.byKey(recordingExportKey));
      await Future<void>.delayed(const Duration(milliseconds: 100));
    });
    await tester.pumpAndSettle();
    expect(find.text('Unable to save the recording'), findsOneWidget);
  });

  testWidgets('without an export controller there is no share button',
      (WidgetTester tester) async {
    await pumpMeeting(tester);
    expect(find.byKey(recordingPlayerBarKey), findsOneWidget);
    expect(find.byKey(recordingExportKey), findsNothing);
  });
}

/// Records what the share sheet was given.
final class _RecordingSharer implements ExportSharer {
  _RecordingSharer(this.directory);

  final Directory directory;
  final List<String> shared = <String>[];

  @override
  Future<String> shareDirectory() async => directory.path;

  @override
  Future<void> shareFiles(List<String> paths, String text) async =>
      shared.addAll(paths);
}

/// A desktop without a save dialog (Linux without zenity, for example).
final class _NoDialogSaver implements ExportSaver {
  @override
  bool get savesBytes => false;

  @override
  Future<String?> chooseSavePath(String suggestedFileName) =>
      throw const FileSystemException('no dialog');

  @override
  Future<bool> saveBytes(String suggestedFileName, Uint8List bytes) =>
      throw UnsupportedError('desktops save by path');
}

/// Meeting exports are not used by these tests.
final class _NoMeetingExports implements ExportBridge {
  @override
  Future<MeetingExportResult> exportMeeting(
          String meetingId, MeetingExportFormat format,
          {String? outputPath, bool includeAttachments = true}) =>
      throw UnimplementedError();
}
