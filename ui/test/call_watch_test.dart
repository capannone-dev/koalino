// Verifies the call watch (#111) over a scripted core: a call becomes an offer only while idle,
// recording starts only on the owner's tap, a recording tied to the call stops when it ends (and
// only then), the settings are honoured, and an unavailable system stops the looks.

import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/call_watch_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/call_watch_bridge.dart';
import 'package:note_taker_ui/presentation/call_offer_banner.dart';

import 'recording_screen_test.dart' show FakeRecordingBridge;

/// Answers the core's call watch from a script of polls.
final class ScriptedCallCore {
  final List<Map<String, Object?>> polls = <Map<String, Object?>>[];
  Map<String, Object?> settings = <String, Object?>{
    'ok': true,
    'offer': true,
    'auto_stop': true,
  };
  final List<Map<String, Object?>> requests = <Map<String, Object?>>[];

  Future<String> call(String function, List<String?> arguments) async {
    expect(function, 'note_taker_call_watch');
    final Map<String, Object?> request =
        jsonDecode(arguments.single!) as Map<String, Object?>;
    requests.add(request);
    switch (request['command']) {
      case 'poll':
        return jsonEncode(polls.isEmpty
            ? <String, Object?>{'ok': true, 'available': true, 'in_call': false}
            : polls.removeAt(0));
      case 'set':
        settings = <String, Object?>{
          ...settings,
          if (request['offer'] != null) 'offer': request['offer'],
          if (request['auto_stop'] != null) 'auto_stop': request['auto_stop'],
        };
        return jsonEncode(settings);
      default:
        return jsonEncode(settings);
    }
  }
}

Map<String, Object?> poll(String? event, {String app = 'Teams'}) =>
    <String, Object?>{
      'ok': true,
      'available': true,
      'in_call': event != 'ended',
      'app': app,
      'event': event,
    };

void main() {
  late ScriptedCallCore core;
  late FakeRecordingBridge recordingBridge;
  late RecordingController recording;
  late CallWatchController watch;

  setUp(() {
    core = ScriptedCallCore();
    recordingBridge = FakeRecordingBridge(startResult: true);
    recording = RecordingController(recordingBridge: recordingBridge);
    watch = CallWatchController(
      bridge: CallWatchBridge(call: core.call),
      recordingController: recording,
      interval: const Duration(hours: 1),
    );
  });

  tearDown(() => watch.dispose());

  test('a call is offered, recorded on accept, and stopped when it ends',
      () async {
    core.polls.add(poll('started'));
    await watch.start();
    expect(watch.offeredApp, 'Teams');
    expect(recordingBridge.startRecordingCallCount, 0,
        reason: 'nothing records without a tap');

    await watch.acceptOffer();
    expect(recordingBridge.requestedTitles, <String?>['Teams call']);
    expect(recording.state, RecordingUiState.recording);
    expect(watch.recordingTiedToCall, isTrue);

    core.polls.add(poll('ended'));
    await watch.pollOnce();
    expect(recordingBridge.lifecycleCommands, <String>['stop']);
    expect(watch.message, contains('Teams call ended'));
  });

  test('a recording the owner started is never stopped by a call', () async {
    await recording.startRecording(title: 'Mine');
    core.polls.addAll(<Map<String, Object?>>[poll('started'), poll('ended')]);
    await watch.start();
    expect(watch.offeredApp, isNull, reason: 'already recording');
    await watch.pollOnce();
    expect(recordingBridge.lifecycleCommands, isEmpty);
  });

  test('with auto-stop off, the recording goes on after the call', () async {
    core.settings = <String, Object?>{
      'ok': true,
      'offer': true,
      'auto_stop': false,
    };
    core.polls.add(poll('started'));
    await watch.start();
    await watch.acceptOffer();
    core.polls.add(poll('ended'));
    await watch.pollOnce();
    expect(recordingBridge.lifecycleCommands, isEmpty);
    expect(recording.state, RecordingUiState.recording);
  });

  test('with the offer off, no offer is shown', () async {
    await watch.start();
    await watch.setSettings(offer: false);
    core.polls.add(poll('started'));
    await watch.pollOnce();
    expect(watch.offeredApp, isNull);
    expect(core.requests.last, <String, Object?>{'command': 'poll'});
  });

  test('a system that cannot tell stops the looks', () async {
    core.polls.add(<String, Object?>{'ok': true, 'available': false});
    await watch.start();
    expect(watch.available, isFalse);
  });

  testWidgets('the banner offers, and Not now dismisses',
      (WidgetTester tester) async {
    core.polls.add(poll('started', app: 'Zoom'));
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: CallOfferBanner(controller: watch, child: const Text('Screens')),
      ),
    ));
    await tester.runAsync(watch.start);
    await tester.pump();
    expect(find.text('Zoom is using the microphone. Record this call?'),
        findsOneWidget);
    await tester.tap(find.text('Not now'));
    await tester.pump();
    expect(find.textContaining('Record this call?'), findsNothing);
    expect(recordingBridge.startRecordingCallCount, 0);
  });

  testWidgets('Record in the banner starts the tied recording',
      (WidgetTester tester) async {
    core.polls.add(poll('started', app: 'Zoom'));
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: CallOfferBanner(controller: watch, child: const Text('Screens')),
      ),
    ));
    await tester.runAsync(watch.start);
    await tester.pump();
    await tester.tap(find.text('Record'));
    await tester.pump();
    await tester.pump();
    expect(recordingBridge.requestedTitles, <String?>['Zoom call']);
    expect(find.textContaining('Record this call?'), findsNothing);
  });
}
