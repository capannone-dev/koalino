// Verifies the call watch (#111) over a scripted core: a call becomes an offer only while idle,
// recording starts only on the owner's tap, a recording tied to the call stops when it ends (and
// only then), the settings are honoured, and an unavailable system stops the looks.

import 'dart:async';
import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/call_watch_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/call_notifier.dart';
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
          if (request['read_names'] != null)
            'read_names': request['read_names'],
        };
        return jsonEncode(settings);
      default:
        return jsonEncode(settings);
    }
  }
}

/// A notification the test answers.
final class FakeCallNotifier implements CallNotifier {
  final List<String> offered = <String>[];
  int withdrawn = 0;
  Completer<CallNotificationAnswer>? pending;

  @override
  Future<CallNotificationAnswer> offer(String app) {
    offered.add(app);
    final Completer<CallNotificationAnswer> answer =
        Completer<CallNotificationAnswer>();
    pending = answer;
    return answer.future;
  }

  @override
  void withdraw() {
    withdrawn += 1;
    final Completer<CallNotificationAnswer>? answer = pending;
    pending = null;
    if (answer != null && !answer.isCompleted) {
      answer.complete(CallNotificationAnswer.dismissed);
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
  TestWidgetsFlutterBinding.ensureInitialized();
  late ScriptedCallCore core;
  late FakeRecordingBridge recordingBridge;
  late RecordingController recording;
  late CallWatchController watch;
  late FakeCallNotifier notifier;

  setUp(() {
    notifier = FakeCallNotifier();
    core = ScriptedCallCore();
    recordingBridge = FakeRecordingBridge(startResult: true);
    recording = RecordingController(recordingBridge: recordingBridge);
    watch = CallWatchController(
      bridge: CallWatchBridge(call: core.call),
      recordingController: recording,
      notifier: notifier,
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

  test('Record in the system notification starts the tied recording', () async {
    core.polls.add(poll('started', app: 'Zoom'));
    await watch.start();
    expect(notifier.offered, <String>['Zoom']);
    notifier.pending!.complete(CallNotificationAnswer.record);
    await pumpEventQueue();
    expect(recordingBridge.requestedTitles, <String?>['Zoom call']);
    expect(watch.offeredApp, isNull);
    expect(watch.recordingTiedToCall, isTrue);
  });

  test('answering in the app withdraws the notification', () async {
    core.polls.add(poll('started'));
    await watch.start();
    watch.dismissOffer();
    await pumpEventQueue();
    expect(notifier.withdrawn, greaterThan(0));
    expect(recordingBridge.startRecordingCallCount, 0);
  });

  test('Not now in the notification clears the banner', () async {
    core.polls.add(poll('started'));
    await watch.start();
    notifier.pending!.complete(CallNotificationAnswer.dismissed);
    await pumpEventQueue();
    expect(watch.offeredApp, isNull);
  });

  test('without a system notification the banner keeps the offer (Windows)',
      () async {
    final CallWatchController bannerOnly = CallWatchController(
      bridge: CallWatchBridge(call: core.call),
      recordingController: recording,
      interval: const Duration(hours: 1),
    );
    addTearDown(bannerOnly.dispose);
    core.polls.add(poll('started', app: 'Edge'));
    await bannerOnly.start();
    await pumpEventQueue();
    expect(bannerOnly.offeredApp, 'Edge');
  });

  test('notify-send gets the offer with its two buttons', () {
    expect(NotifySendCallNotifier.argumentsFor('Teams'), <String>[
      '--app-name=Koalino',
      '--icon=audio-input-microphone',
      '--expire-time=120000',
      '--action=record=Record',
      '--action=dismiss=Not now',
      'Teams is using the microphone',
      'Record this call?',
    ]);
  });

  test('the Windows toast asks the core and maps its answers', () async {
    final List<Map<String, Object?>> requests = <Map<String, Object?>>[];
    String answer = 'record';
    final CoreToastCallNotifier toast = CoreToastCallNotifier(
        call: (String function, List<String?> arguments) async {
      expect(function, 'note_taker_call_toast');
      final Map<String, Object?> request =
          jsonDecode(arguments.single!) as Map<String, Object?>;
      requests.add(request);
      return request['command'] == 'offer'
          ? jsonEncode(<String, Object?>{'ok': true, 'answer': answer})
          : jsonEncode(<String, Object?>{'ok': true});
    });

    expect(await toast.offer('Teams'), CallNotificationAnswer.record);
    expect(
        requests.single, <String, Object?>{'command': 'offer', 'app': 'Teams'});
    answer = 'dismissed';
    expect(await toast.offer('Teams'), CallNotificationAnswer.dismissed);
    answer = 'open';
    expect(await toast.offer('Teams'), CallNotificationAnswer.unavailable,
        reason: 'a click on the toast itself leaves the banner up');
    answer = 'unavailable';
    expect(await toast.offer('Teams'), CallNotificationAnswer.unavailable);
    toast.withdraw();
    await pumpEventQueue();
    expect(requests.last, <String, Object?>{'command': 'withdraw'});
  });

  test('a core without the toast leaves the banner up', () async {
    final CoreToastCallNotifier toast = CoreToastCallNotifier(
        call: (String function, List<String?> arguments) async =>
            throw ArgumentError('missing symbol'));
    expect(await toast.offer('Teams'), CallNotificationAnswer.unavailable);
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

  test('reading names can be turned off, and a needed restart is told (#113)',
      () async {
    core.settings = <String, Object?>{
      'ok': true,
      'offer': true,
      'auto_stop': true,
      'read_names': true,
      'restart_apps': true,
    };
    await watch.start();
    expect(watch.settings.readNames, isTrue);
    expect(watch.message, contains('Restart'));
    watch.clearMessage();
    await watch.setSettings(readNames: false);
    expect(core.requests.last,
        <String, Object?>{'command': 'set', 'read_names': false});
    expect(watch.settings.readNames, isFalse);
  });

  test('a failed look keeps the watch going', () async {
    core.polls.addAll(<Map<String, Object?>>[
      <String, Object?>{'ok': false, 'error_code': 'probe_failed'},
      poll('started'),
    ]);
    await watch.start();
    expect(watch.offeredApp, isNull);
    await watch.pollOnce();
    expect(watch.offeredApp, 'Teams');
  });

  test('a system that cannot tell stops the looks', () async {
    core.polls.add(<String, Object?>{'ok': true, 'available': false});
    await watch.start();
    expect(watch.available, isFalse);
  });

  test('a host that sees the users sends them with each look (Android)',
      () async {
    final CallWatchController android = CallWatchController(
      bridge: CallWatchBridge(call: core.call),
      recordingController: recording,
      notifier: notifier,
      hostUsers: () async => <HostMicrophoneUser>[
        const HostMicrophoneUser(process: 'android:voice-communication'),
      ],
      interval: const Duration(hours: 1),
    );
    addTearDown(android.dispose);
    core.polls.add(poll('started', app: 'A call app'));
    await android.start();

    final Map<String, Object?> sent = core.requests.lastWhere(
        (Map<String, Object?> request) => request['command'] == 'poll');
    expect(sent['users'], <Object?>[
      <String, Object?>{'process': 'android:voice-communication', 'name': ''},
    ]);
    expect(android.offeredApp, 'A call app');
    await android.acceptOffer();
    expect(recordingBridge.requestedTitles, <String?>['Call'],
        reason: 'an unnamed call app is not "A call app call"');
  });

  test('a host that cannot answer skips the look', () async {
    final CallWatchController android = CallWatchController(
      bridge: CallWatchBridge(call: core.call),
      recordingController: recording,
      hostUsers: () async => throw PlatformException(code: 'gone'),
      interval: const Duration(hours: 1),
    );
    addTearDown(android.dispose);
    await android.start();
    expect(
        core.requests.where(
            (Map<String, Object?> request) => request['command'] == 'poll'),
        isEmpty);
  });

  test('only desktops offer to record calls: Android silences a second capture',
      () {
    expect(offersToRecordCalls('linux'), isTrue);
    expect(offersToRecordCalls('windows'), isTrue);
    expect(offersToRecordCalls('android'), isFalse);
    expect(offersToRecordCalls('macos'), isFalse);
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
