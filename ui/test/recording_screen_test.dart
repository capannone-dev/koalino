// Defines widget-first recording-state and meeting-title requirements for the Flutter client.

// ignore: deprecated_member_use
import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_attachments_controller.dart';
import 'package:note_taker_ui/controllers/recording_controller.dart';
import 'package:note_taker_ui/infrastructure/microphone_hearing.dart';
import 'package:note_taker_ui/infrastructure/microphone_permission.dart';
import 'package:note_taker_ui/infrastructure/recording_bridge.dart';
import 'package:note_taker_ui/infrastructure/recording_notifier.dart';
import 'package:note_taker_ui/presentation/recording_screen.dart';

import 'fake_photo_camera.dart';

/// Records calls made by a widget through the bridge boundary without loading Rust code.
final class FakeRecordingBridge implements RecordingBridge {
  /// Creates a fake that returns the supplied outcomes.
  FakeRecordingBridge({
    required this.startResult,
    this.pauseResult = true,
    this.resumeResult = true,
    this.stopResult = true,
    this.startOutcome,
    this.resumeOutcome,
  });

  /// Controls whether the simulated Rust start operation succeeds.
  final bool startResult;

  /// Overrides the start outcome derived from `startResult` (e.g. microphone unavailable).
  final RecordingStartOutcome? startOutcome;

  /// Overrides the resume outcome derived from `resumeResult`.
  final RecordingStartOutcome? resumeOutcome;

  /// Controls whether `pauseRecording` succeeds.
  final bool pauseResult;

  /// Controls whether `startRecording` (resume) succeeds.
  final bool resumeResult;

  /// Controls whether `stopRecording` succeeds.
  final bool stopResult;

  /// When set, `stopRecording` waits for this completer, like Rust finishing transcription.
  Completer<void>? stopGate;

  /// Lifecycle commands after the first start, in order: `pause`, `resume`, `stop`.
  final List<String> lifecycleCommands = <String>[];

  /// Titles passed to `startRecordingWithTitle`, in call order (`null` for untitled starts).
  final List<String?> requestedTitles = <String?>[];

  /// Counts the recording-start requests made through this bridge.
  int get startRecordingCallCount => requestedTitles.length;

  /// The meeting `activeMeetingId` reports, like Rust while a recording runs.
  String? recordingMeetingId;

  @override
  Future<String?> activeMeetingId() async => recordingMeetingId;

  @override
  Future<bool> setLogLevel(String filterDirective) async => true;

  @override
  Future<RecordingStartOutcome> startRecording() async {
    lifecycleCommands.add('resume');
    return resumeOutcome ?? _outcomeOf(resumeResult);
  }

  @override
  Future<bool> pauseRecording() async {
    lifecycleCommands.add('pause');
    return pauseResult;
  }

  @override
  Future<bool> stopRecording() async {
    lifecycleCommands.add('stop');
    await stopGate?.future;
    return stopResult;
  }

  @override
  Future<RecordingStartOutcome> startRecordingWithTitle(String? title) async {
    requestedTitles.add(title);
    return startOutcome ?? _outcomeOf(startResult);
  }

  static RecordingStartOutcome _outcomeOf(bool succeeded) =>
      succeeded ? RecordingStartOutcome.started : RecordingStartOutcome.failed;
}

/// Answers microphone permission requests with a fixed result and counts them.
final class FakeMicrophonePermission implements MicrophonePermission {
  /// Creates a fake that grants or denies every request.
  FakeMicrophonePermission({required this.granted});

  /// Whether requests are granted.
  final bool granted;

  /// How many times the permission was requested.
  int requestCount = 0;

  @override
  Future<bool> ensureGranted() async {
    requestCount += 1;
    return granted;
  }
}

/// Records what the controller shows in the notification and replays notification actions.
final class FakeRecordingNotifier implements RecordingNotifier {
  /// Creates a fake whose service starts unless `serviceStarts` is false.
  FakeRecordingNotifier({this.serviceStarts = true, this.bridge});

  /// Whether the simulated foreground service starts (Android may refuse it).
  final bool serviceStarts;

  /// The bridge whose start requests are counted at each `show`, when set.
  final FakeRecordingBridge? bridge;

  /// Every state shown, in order, with its title.
  final List<(RecordingNotificationState, String?)> shown =
      <(RecordingNotificationState, String?)>[];

  /// Rust start and resume requests made before each `show`, in order (needs `bridge`).
  final List<int> rustStartsAtShow = <int>[];

  final StreamController<RecordingNotificationState> _actions =
      StreamController<RecordingNotificationState>.broadcast();

  @override
  Future<bool> show(RecordingNotificationState state, {String? title}) async {
    shown.add((state, title));
    final FakeRecordingBridge? watchedBridge = bridge;
    if (watchedBridge != null) {
      rustStartsAtShow.add(watchedBridge.startRecordingCallCount +
          watchedBridge.lifecycleCommands
              .where((String command) => command == 'resume')
              .length);
    }
    return serviceStarts;
  }

  @override
  Stream<RecordingNotificationState> get externalChanges => _actions.stream;

  /// Simulates the notification (or lock screen) changing the recording.
  void emit(RecordingNotificationState state) => _actions.add(state);
}

/// Pumps the recording screen around a controller backed by `recordingBridge`.
Future<void> pumpRecordingScreen(
    WidgetTester tester, FakeRecordingBridge recordingBridge,
    {MicrophonePermission? microphonePermission,
    RecordingNotifier? recordingNotifier}) async {
  await tester.pumpWidget(
    MaterialApp(
      home: RecordingScreen(
        recordingController: RecordingController(
          recordingBridge: recordingBridge,
          microphonePermission:
              microphonePermission ?? const GrantedMicrophonePermission(),
          recordingNotifier: recordingNotifier ?? const NoRecordingNotifier(),
        ),
      ),
    ),
  );
}

/// Pumps the recording screen with the camera action, recording which meetings it opened.
Future<List<String>> pumpRecordingScreenWithCamera(
    WidgetTester tester, FakeRecordingBridge recordingBridge) async {
  final List<String> photoScreenMeetings = <String>[];
  await tester.pumpWidget(
    MaterialApp(
      home: RecordingScreen(
        recordingController: RecordingController(
          recordingBridge: recordingBridge,
          microphonePermission: const GrantedMicrophonePermission(),
          recordingNotifier: const NoRecordingNotifier(),
        ),
        activeMeetingId: recordingBridge.activeMeetingId,
        photoScreenBuilder: (BuildContext context, String meetingId) {
          photoScreenMeetings.add(meetingId);
          return const Scaffold(body: Text('Camera'));
        },
      ),
    ),
  );
  return photoScreenMeetings;
}

/// Verifies the recording lifecycle and the optional meeting title sent to Rust.
void main() {
  testWidgets(
      'while recording a timer counts the recorded time and holds while paused (#132)',
      (WidgetTester tester) async {
    DateTime now = DateTime.utc(2026, 11, 17, 10);
    final RecordingController controller = RecordingController(
      recordingBridge: FakeRecordingBridge(startResult: true),
      now: () => now,
    );
    await tester.pumpWidget(
        MaterialApp(home: RecordingScreen(recordingController: controller)));
    expect(find.byKey(const ValueKey<String>('recording-timer')), findsNothing);

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    expect(find.text('0:00:00'), findsOneWidget);
    now = now.add(const Duration(minutes: 1, seconds: 5));
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('0:01:05'), findsOneWidget);

    await tester.tap(find.text('Pause'));
    await tester.pump();
    now = now.add(const Duration(minutes: 10));
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('0:01:05'), findsOneWidget,
        reason: 'a pause records nothing');

    await tester.tap(find.text('Resume'));
    await tester.pump();
    now = now.add(const Duration(hours: 1, seconds: 2));
    await tester.pump(const Duration(seconds: 1));
    expect(find.text('1:01:07'), findsOneWidget);

    await tester.tap(find.text('Stop'));
    await tester.pumpAndSettle();
    expect(find.byKey(const ValueKey<String>('recording-timer')), findsNothing);
    controller.dispose();
  });

  testWidgets('ready, recording and paused look clearly different (#132)',
      (WidgetTester tester) async {
    final RecordingController controller = RecordingController(
        recordingBridge: FakeRecordingBridge(startResult: true));
    await tester.pumpWidget(
        MaterialApp(home: RecordingScreen(recordingController: controller)));
    final ColorScheme colors =
        Theme.of(tester.element(find.byType(RecordingScreen))).colorScheme;
    Color? panelColor(String state) => tester
        .widget<Card>(find.byKey(ValueKey<String>('recording-status-$state')))
        .color;

    // Ready is the start button, in the primary container color.
    expect(panelColor('idle'), colors.primaryContainer);

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    expect(panelColor('recording'), colors.errorContainer);
    expect(find.text('REC'), findsOneWidget);

    await tester.tap(find.text('Pause'));
    await tester.pump();
    expect(panelColor('paused'), colors.tertiaryContainer);
    expect(find.text('REC'), findsNothing);

    await tester.tap(find.text('Stop'));
    await tester.pumpAndSettle();
    controller.dispose();
  });

  testWidgets(
      'a microphone Android silences for a call is said while recording',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    bool silenced = true;
    int checks = 0;
    final RecordingController controller = RecordingController(
      recordingBridge: recordingBridge,
      microphoneSilenced: () async {
        checks += 1;
        return silenced;
      },
      silenceCheckInterval: const Duration(seconds: 2),
    );
    await tester.pumpWidget(
        MaterialApp(home: RecordingScreen(recordingController: controller)));
    await tester.pump(const Duration(seconds: 5));
    expect(checks, 0, reason: 'nothing is checked while idle');

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    await tester.pump(const Duration(seconds: 2));
    expect(controller.microphoneSilenced, isTrue);
    expect(find.textContaining('silencing the microphone'), findsOneWidget);

    silenced = false;
    await tester.pump(const Duration(seconds: 2));
    expect(find.textContaining('silencing the microphone'), findsNothing);

    await tester.tap(find.text('Stop'));
    await tester.pumpAndSettle();
    final int checksAtStop = checks;
    await tester.pump(const Duration(seconds: 10));
    expect(checks, checksAtStop, reason: 'checks end with the recording');
    controller.dispose();
  });

  testWidgets('a muted or turned-off microphone is said while recording',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    MicrophoneHearing hearing = MicrophoneHearing.silent;
    final RecordingController controller = RecordingController(
      recordingBridge: recordingBridge,
      microphoneHearing: () async => hearing,
      silenceCheckInterval: const Duration(seconds: 2),
    );
    await tester.pumpWidget(
        MaterialApp(home: RecordingScreen(recordingController: controller)));

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    await tester.pump(const Duration(seconds: 2));
    expect(controller.microphoneHearing, MicrophoneHearing.silent);
    expect(find.textContaining('The microphone seems muted or turned off'),
        findsOneWidget);

    hearing = MicrophoneHearing.blockedByWindows;
    await tester.pump(const Duration(seconds: 2));
    expect(find.textContaining('Windows is keeping apps from the microphone'),
        findsOneWidget);

    hearing = MicrophoneHearing.heard;
    await tester.pump(const Duration(seconds: 2));
    expect(find.textContaining('muted or turned off'), findsNothing);
    expect(find.textContaining('Windows is keeping'), findsNothing);

    await tester.tap(find.text('Stop'));
    await tester.pumpAndSettle();
    controller.dispose();
  });

  testWidgets('a call silencing the microphone is said instead of a mute',
      (WidgetTester tester) async {
    final RecordingController controller = RecordingController(
      recordingBridge: FakeRecordingBridge(startResult: true),
      microphoneSilenced: () async => true,
      microphoneHearing: () async => MicrophoneHearing.silent,
      silenceCheckInterval: const Duration(seconds: 2),
    );
    await tester.pumpWidget(
        MaterialApp(home: RecordingScreen(recordingController: controller)));

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    await tester.pump(const Duration(seconds: 2));

    expect(find.textContaining('silencing the microphone'), findsOneWidget);
    expect(find.textContaining('muted or turned off'), findsNothing,
        reason: 'the call explains the silence');
    await tester.tap(find.text('Stop'));
    await tester.pumpAndSettle();
    controller.dispose();
  });

  testWidgets('Take photos opens the camera for the meeting being recorded',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    final List<String> photoScreenMeetings =
        await pumpRecordingScreenWithCamera(tester, recordingBridge);
    expect(find.text('Take photos'), findsNothing, reason: 'idle');

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    recordingBridge.recordingMeetingId = 'meeting-live';
    await tester.tap(find.text('Take photos'));
    await tester.pumpAndSettle();

    expect(find.text('Camera'), findsOneWidget);
    expect(photoScreenMeetings, <String>['meeting-live']);
  });

  testWidgets('Take photos explains when the meeting is not known yet',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    final List<String> photoScreenMeetings =
        await pumpRecordingScreenWithCamera(tester, recordingBridge);

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    await tester.tap(find.text('Take photos'));
    await tester.pumpAndSettle();

    expect(photoScreenMeetings, isEmpty);
    expect(find.text('The recording is still starting; try again.'),
        findsOneWidget);
  });

  testWidgets('Attach files while recording attaches to the recorded meeting',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    final FakeMeetingAttachmentBridge attachmentBridge =
        FakeMeetingAttachmentBridge();
    final FakeFilesPicker filesPicker = FakeFilesPicker()
      ..pickedPaths = <String>['/tmp/slides.pdf'];
    await tester.pumpWidget(
      MaterialApp(
        home: RecordingScreen(
          recordingController: RecordingController(
            recordingBridge: recordingBridge,
            microphonePermission: const GrantedMicrophonePermission(),
            recordingNotifier: const NoRecordingNotifier(),
          ),
          activeMeetingId: recordingBridge.activeMeetingId,
          attachmentsController: MeetingAttachmentsController(
              attachmentBridge: attachmentBridge, filesPicker: filesPicker),
        ),
      ),
    );
    expect(find.text('Attach files'), findsNothing, reason: 'idle');

    await tester.tap(find.text('Start recording'));
    await tester.pump();
    recordingBridge.recordingMeetingId = 'meeting-live';
    await tester.tap(find.text('Attach files'));
    await tester.pumpAndSettle();

    expect(attachmentBridge.requests.single.meetingId, 'meeting-live');
    expect(find.text('Attached 1 file'), findsOneWidget);
  });

  testWidgets('without a camera the recording screen has no Take photos',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(find.text('Take photos'), findsNothing);
  });

  testWidgets(
      'start control changes idle state to recording after bridge success',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    // The status panel is the start button; there is no separate one.
    expect(find.text('Start recording'), findsOneWidget);
    expect(find.text('Ready to record'), findsNothing);
    expect(find.byType(FilledButton), findsNothing);
    final Finder panel =
        find.byKey(const ValueKey<String>('recording-status-idle'));
    expect(
        tester.getSemantics(panel),
        matchesSemantics(
            label: 'Start recording',
            isButton: true,
            hasTapAction: true,
            hasEnabledState: true,
            isEnabled: true,
            isFocusable: true,
            hasFocusAction: true));

    await tester.tap(panel);
    await tester.pump();

    expect(recordingBridge.startRecordingCallCount, 1);
    expect(find.text('Recording'), findsOneWidget);
  });

  testWidgets('the start panel works from the keyboard',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    final Finder panel =
        find.byKey(const ValueKey<String>('recording-status-idle'));
    final Finder panelFocus =
        find.descendant(of: panel, matching: find.byType(Focus)).first;
    Focus.of(tester.element(find.descendant(
            of: panelFocus, matching: find.text('Start recording'))))
        .requestFocus();
    await tester.pump();
    await tester.sendKeyEvent(LogicalKeyboardKey.enter);
    await tester.pump();

    expect(recordingBridge.startRecordingCallCount, 1);
  });

  testWidgets('failed bridge start keeps the recorder idle and shows an error',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: false);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.startRecordingCallCount, 1);
    expect(find.text('Start recording'), findsOneWidget);
    expect(find.text('Unable to start recording'), findsOneWidget);
  });

  testWidgets('a microphone that cannot open keeps the recorder idle',
      (WidgetTester tester) async {
    await pumpRecordingScreen(
        tester,
        FakeRecordingBridge(
            startResult: false,
            startOutcome: RecordingStartOutcome.microphoneUnavailable));

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(find.text('Start recording'), findsOneWidget);
    expect(find.text('Unable to open the microphone'), findsOneWidget);
  });

  testWidgets('a denied microphone permission never starts a recording',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    final FakeMicrophonePermission microphonePermission =
        FakeMicrophonePermission(granted: false);
    await pumpRecordingScreen(tester, recordingBridge,
        microphonePermission: microphonePermission);

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(microphonePermission.requestCount, 1);
    expect(recordingBridge.startRecordingCallCount, 0);
    expect(find.text('Start recording'), findsOneWidget);
    expect(find.text('Microphone access is needed to record'), findsOneWidget);
  });

  testWidgets('an optional meeting title field exists and accepts text',
      (WidgetTester tester) async {
    await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true));

    final Finder titleField = find.byType(TextField);
    expect(titleField, findsOneWidget);
    expect(find.text('Meeting title'), findsOneWidget);
    expect(find.text('Enter meeting title (optional)'), findsOneWidget);

    await tester.enterText(titleField, 'Project Sync');
    expect(find.text('Project Sync'), findsOneWidget);
  });

  testWidgets('starting sends the trimmed entered title to the bridge',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.enterText(find.byType(TextField), '  Project Sync  ');
    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.requestedTitles, <String?>['Project Sync']);
    expect(find.text('Recording "Project Sync"'), findsOneWidget);
  });

  testWidgets('a blank title is sent as null so Rust uses its fallback',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.enterText(find.byType(TextField), '   ');
    await tester.tap(find.text('Start recording'));
    await tester.pump();

    expect(recordingBridge.requestedTitles, <String?>[null]);
    expect(find.text('Recording'), findsOneWidget);
  });

  testWidgets('submitting the title from the keyboard starts the recording',
      (WidgetTester tester) async {
    final FakeRecordingBridge recordingBridge =
        FakeRecordingBridge(startResult: true);
    await pumpRecordingScreen(tester, recordingBridge);

    await tester.enterText(find.byType(TextField), 'Retro');
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pump();

    expect(recordingBridge.requestedTitles, <String?>['Retro']);
  });

  testWidgets('the title field is locked while recording',
      (WidgetTester tester) async {
    await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true));

    await tester.tap(find.text('Start recording'));
    await tester.pump();

    final TextField titleField =
        tester.widget<TextField>(find.byType(TextField));
    expect(titleField.enabled, isFalse);
  });

  testWidgets('the title field fits a phone and is width-capped on desktop',
      (WidgetTester tester) async {
    addTearDown(tester.view.reset);
    tester.view.devicePixelRatio = 1;

    tester.view.physicalSize = const Size(360, 640);
    await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true));
    expect(tester.takeException(), isNull);
    expect(
        tester.getSize(find.byType(TextField)).width, lessThanOrEqualTo(360));

    tester.view.physicalSize = const Size(1440, 900);
    await tester.pump();
    expect(
        tester.getSize(find.byType(TextField)).width, lessThanOrEqualTo(480));
  });

  group('pause, resume, and stop', () {
    /// Pumps the screen and starts a titled recording.
    Future<FakeRecordingBridge> startRecordingIn(WidgetTester tester,
        {bool pauseResult = true,
        bool stopResult = true,
        RecordingStartOutcome? resumeOutcome}) async {
      final FakeRecordingBridge recordingBridge = FakeRecordingBridge(
          startResult: true,
          pauseResult: pauseResult,
          stopResult: stopResult,
          resumeOutcome: resumeOutcome);
      await pumpRecordingScreen(tester, recordingBridge);
      await tester.enterText(find.byType(TextField), 'Weekly sync');
      await tester.tap(find.text('Start recording'));
      await tester.pump();
      return recordingBridge;
    }

    testWidgets('a resume whose microphone cannot open stays paused',
        (WidgetTester tester) async {
      await startRecordingIn(tester,
          resumeOutcome: RecordingStartOutcome.microphoneUnavailable);

      await tester.tap(find.text('Pause'));
      await tester.pump();
      await tester.tap(find.text('Resume'));
      await tester.pump();

      expect(find.text('Resume'), findsOneWidget);
      expect(find.text('Unable to open the microphone'), findsOneWidget);
    });

    testWidgets('a recording offers Pause and Stop instead of Start',
        (WidgetTester tester) async {
      await startRecordingIn(tester);

      expect(find.text('Pause'), findsOneWidget);
      expect(find.text('Stop'), findsOneWidget);
      expect(find.text('Start recording'), findsNothing);
    });

    testWidgets('Pause pauses the recording and offers Resume',
        (WidgetTester tester) async {
      final FakeRecordingBridge recordingBridge =
          await startRecordingIn(tester);

      await tester.tap(find.text('Pause'));
      await tester.pump();

      expect(recordingBridge.lifecycleCommands, <String>['pause']);
      expect(find.text('Paused "Weekly sync"'), findsOneWidget);
      expect(find.text('Resume'), findsOneWidget);
      expect(find.text('Stop'), findsOneWidget);
    });

    testWidgets(
        'Resume continues the same meeting instead of starting a new one',
        (WidgetTester tester) async {
      final FakeRecordingBridge recordingBridge =
          await startRecordingIn(tester);
      await tester.tap(find.text('Pause'));
      await tester.pump();

      await tester.tap(find.text('Resume'));
      await tester.pump();

      expect(recordingBridge.lifecycleCommands, <String>['pause', 'resume']);
      expect(recordingBridge.startRecordingCallCount, 1);
      expect(find.text('Recording "Weekly sync"'), findsOneWidget);
    });

    testWidgets('Stop waits for transcription, then returns to idle',
        (WidgetTester tester) async {
      final FakeRecordingBridge recordingBridge =
          await startRecordingIn(tester);
      recordingBridge.stopGate = Completer<void>();

      await tester.tap(find.text('Stop'));
      await tester.pump();

      expect(find.text('Finishing transcription…'), findsOneWidget);
      expect(find.byType(LinearProgressIndicator), findsOneWidget);
      final ButtonStyleButton stopButton = tester.widget<ButtonStyleButton>(
          find.ancestor(
              of: find.text('Stop'),
              matching: find.byWidgetPredicate(
                  (Widget widget) => widget is ButtonStyleButton)));
      expect(stopButton.onPressed, isNull);

      recordingBridge.stopGate?.complete();
      await tester.pump();

      expect(recordingBridge.lifecycleCommands, <String>['stop']);
      expect(find.text('Start recording'), findsOneWidget);
      expect(find.text('Recording saved'), findsOneWidget);
      final TextField titleField =
          tester.widget<TextField>(find.byType(TextField));
      expect(titleField.enabled, isTrue);
      expect(titleField.controller?.text, isEmpty);
    });

    testWidgets('a paused recording can be stopped',
        (WidgetTester tester) async {
      final FakeRecordingBridge recordingBridge =
          await startRecordingIn(tester);
      await tester.tap(find.text('Pause'));
      await tester.pump();

      await tester.tap(find.text('Stop'));
      await tester.pump();

      expect(recordingBridge.lifecycleCommands, <String>['pause', 'stop']);
      expect(find.text('Start recording'), findsOneWidget);
    });

    testWidgets('a failed pause keeps recording and shows an error',
        (WidgetTester tester) async {
      await startRecordingIn(tester, pauseResult: false);

      await tester.tap(find.text('Pause'));
      await tester.pump();

      expect(find.text('Recording "Weekly sync"'), findsOneWidget);
      expect(find.text('Unable to pause recording'), findsOneWidget);
    });

    testWidgets('a failed stop keeps the recording and shows an error',
        (WidgetTester tester) async {
      await startRecordingIn(tester, stopResult: false);

      await tester.tap(find.text('Stop'));
      await tester.pump();

      expect(find.text('Recording "Weekly sync"'), findsOneWidget);
      expect(find.text('Unable to stop recording'), findsOneWidget);
      expect(find.text('Stop'), findsOneWidget);
    });
  });

  group('notification and lock-screen controls', () {
    /// Pumps the screen with `notifier` and starts a recording titled "Weekly sync".
    Future<FakeRecordingBridge> startWithNotifier(
        WidgetTester tester, FakeRecordingNotifier notifier) async {
      final FakeRecordingBridge recordingBridge =
          FakeRecordingBridge(startResult: true);
      await pumpRecordingScreen(tester, recordingBridge,
          recordingNotifier: notifier);
      await tester.enterText(find.byType(TextField), 'Weekly sync');
      await tester.tap(find.text('Start recording'));
      await tester.pump();
      return recordingBridge;
    }

    testWidgets('the notification follows start, pause, resume, and stop',
        (WidgetTester tester) async {
      final FakeRecordingNotifier notifier = FakeRecordingNotifier();
      await startWithNotifier(tester, notifier);

      await tester.tap(find.text('Pause'));
      await tester.pump();
      await tester.tap(find.text('Resume'));
      await tester.pump();
      await tester.tap(find.text('Stop'));
      await tester.pump();

      expect(notifier.shown, <(RecordingNotificationState, String?)>[
        (RecordingNotificationState.recording, 'Weekly sync'),
        (RecordingNotificationState.paused, 'Weekly sync'),
        (RecordingNotificationState.recording, 'Weekly sync'),
        (RecordingNotificationState.stopping, 'Weekly sync'),
        (RecordingNotificationState.idle, null),
      ]);
    });

    testWidgets('a failed start ends the notification it started',
        (WidgetTester tester) async {
      final FakeRecordingNotifier notifier = FakeRecordingNotifier();
      await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: false),
          recordingNotifier: notifier);

      await tester.tap(find.text('Start recording'));
      await tester.pump();

      expect(notifier.shown, <(RecordingNotificationState, String?)>[
        (RecordingNotificationState.recording, null),
        (RecordingNotificationState.idle, null),
      ]);
    });

    // Android silences the microphone of an app that leaves the screen without a running
    // foreground service, and refuses to start one from the background. Rust can take seconds to
    // start (model loading), so the service must start at the tap, before Rust opens the mic.
    testWidgets('the service starts before Rust opens the microphone',
        (WidgetTester tester) async {
      final FakeRecordingBridge recordingBridge =
          FakeRecordingBridge(startResult: true);
      final FakeRecordingNotifier notifier =
          FakeRecordingNotifier(bridge: recordingBridge);
      await pumpRecordingScreen(tester, recordingBridge,
          recordingNotifier: notifier);

      await tester.tap(find.text('Start recording'));
      await tester.pump();
      await tester.tap(find.text('Pause'));
      await tester.pump();
      await tester.tap(find.text('Resume'));
      await tester.pump();

      expect(
          notifier.shown.map(((RecordingNotificationState, String?) s) => s.$1),
          <RecordingNotificationState>[
            RecordingNotificationState.recording,
            RecordingNotificationState.paused,
            RecordingNotificationState.recording,
          ]);
      expect(notifier.rustStartsAtShow, <int>[0, 1, 1],
          reason:
              'each recording state is shown before Rust starts or resumes');
    });

    testWidgets('a microphone denial shows no notification',
        (WidgetTester tester) async {
      final FakeRecordingNotifier notifier = FakeRecordingNotifier();
      await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true),
          recordingNotifier: notifier,
          microphonePermission: FakeMicrophonePermission(granted: false));

      await tester.tap(find.text('Start recording'));
      await tester.pump();

      expect(notifier.shown, isEmpty);
    });

    testWidgets('a failed resume shows the recording paused again',
        (WidgetTester tester) async {
      final FakeRecordingBridge recordingBridge =
          FakeRecordingBridge(startResult: true, resumeResult: false);
      final FakeRecordingNotifier notifier = FakeRecordingNotifier();
      await pumpRecordingScreen(tester, recordingBridge,
          recordingNotifier: notifier);
      await tester.tap(find.text('Start recording'));
      await tester.pump();
      await tester.tap(find.text('Pause'));
      await tester.pump();

      await tester.tap(find.text('Resume'));
      await tester.pump();

      expect(notifier.shown.last.$1, RecordingNotificationState.paused);
    });

    testWidgets('a refused service tells the owner to keep the app open',
        (WidgetTester tester) async {
      final FakeRecordingNotifier notifier =
          FakeRecordingNotifier(serviceStarts: false);
      await pumpRecordingScreen(tester, FakeRecordingBridge(startResult: true),
          recordingNotifier: notifier);

      await tester.tap(find.text('Start recording'));
      await tester.pump();

      expect(find.text('Recording'), findsOneWidget);
      expect(find.text(backgroundRecordingRefusedMessage), findsOneWidget);
    });

    testWidgets('a pause from the notification shows the recording paused',
        (WidgetTester tester) async {
      final FakeRecordingNotifier notifier = FakeRecordingNotifier();
      final FakeRecordingBridge recordingBridge =
          await startWithNotifier(tester, notifier);

      notifier.emit(RecordingNotificationState.paused);
      await tester.pump();

      expect(find.text('Paused "Weekly sync"'), findsOneWidget);
      expect(recordingBridge.lifecycleCommands, isEmpty,
          reason: 'the notification already paused Rust');

      notifier.emit(RecordingNotificationState.recording);
      await tester.pump();

      expect(find.text('Recording "Weekly sync"'), findsOneWidget);
    });

    testWidgets('a stop from the notification ends with Recording saved',
        (WidgetTester tester) async {
      final FakeRecordingNotifier notifier = FakeRecordingNotifier();
      final FakeRecordingBridge recordingBridge =
          await startWithNotifier(tester, notifier);

      notifier.emit(RecordingNotificationState.stopping);
      await tester.pump();
      expect(find.text('Finishing transcription…'), findsOneWidget);

      notifier.emit(RecordingNotificationState.idle);
      await tester.pump();

      expect(find.text('Start recording'), findsOneWidget);
      expect(find.text('Recording saved'), findsOneWidget);
      expect(recordingBridge.lifecycleCommands, isEmpty);
    });
  });
}
