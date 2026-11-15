// Specifies handing a meeting to the owner's own AI app (#114): the share carries the prompt and
// files, a summary's answer copied in that app is kept when Koalino comes back, with Undo, and a
// chat leaves nothing pending. An answer shared back to Koalino as text is offered as the
// summary too.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/handoff_controller.dart';
import 'package:note_taker_ui/controllers/projects_controller.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/infrastructure/handoff_bridge.dart';
import 'package:note_taker_ui/infrastructure/incoming_share_bridge.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/project_bridge.dart';
import 'package:note_taker_ui/presentation/chat_screen.dart';
import 'package:note_taker_ui/presentation/handoff_widgets.dart';
import 'package:note_taker_ui/presentation/share_to_project.dart';

import 'fake_meeting_bridge.dart';
import 'fake_project_runner.dart';

/// One text share, handed out once, with its folder released.
final class _TextShareBridge implements IncomingShareBridge {
  _TextShareBridge(this.share);

  IncomingShare? share;
  final List<String> released = <String>[];

  @override
  Stream<void> get arrivals => const Stream<void>.empty();

  @override
  Future<IncomingShare?> takePending() async {
    final IncomingShare? taken = share;
    share = null;
    return taken;
  }

  @override
  Future<void> release(IncomingShare share) async =>
      released.add(share.directory);
}

/// A picker that is never used here.
final class _NoPicker implements ProjectFilesPicker {
  const _NoPicker();

  @override
  Future<List<String>> pickFiles() async => const <String>[];
}

/// Records what the controller asks of the core and the phone.
final class FakeHandoffPorts implements HandoffCore, HandoffPlatform {
  final List<String> calls = <String>[];
  String? clipboard;
  List<AiApp> apps = <AiApp>[AiApp.claude];
  Map<String, Object?>? previousSummary;
  String? failure;

  @override
  Future<PreparedHandoff> prepare(
      String meetingId, HandoffPurpose purpose, String directory) async {
    if (failure case final String code) {
      throw LlmException(code);
    }
    calls.add('prepare $meetingId ${purpose.name} $directory');
    return PreparedHandoff(
      message: purpose == HandoffPurpose.summary ? 'Summarize it.' : 'Read it.',
      prompt: purpose == HandoffPurpose.summary ? 'Summarize it.' : '',
      files: const <HandoffFile>[
        HandoffFile(
            path: '/cache/shared/handoff/Review.md', mimeType: 'text/markdown'),
      ],
      skipped: 0,
    );
  }

  @override
  Future<PreparedHandoff> prepareProject(
      String projectId, String directory) async {
    calls.add('prepare project $projectId $directory');
    return const PreparedHandoff(
      message: 'Read it.',
      prompt: '',
      files: <HandoffFile>[
        HandoffFile(
            path: '/cache/shared/handoff/Launch.md', mimeType: 'text/markdown'),
        HandoffFile(
            path: '/cache/shared/handoff/plan.pdf',
            mimeType: 'application/pdf'),
      ],
      skipped: 0,
    );
  }

  @override
  Future<KeptAnswer> keepAnswer(String meetingId,
      {required String text,
      required String source,
      required String prompt}) async {
    calls.add('keep $meetingId $source "$prompt" ${text.length}');
    return KeptAnswer(
      summary: StoredSummary(
        text: text,
        connectionLabel: source,
        model: '',
        createdAt: DateTime.utc(2026, 11, 12),
        edited: true,
        truncated: false,
      ),
      previous: previousSummary,
    );
  }

  @override
  Future<void> restore(
          String meetingId, Map<String, Object?>? previous) async =>
      calls.add('restore $meetingId ${previous?['text']}');

  @override
  bool viaBrowser = false;

  /// Whether the website opens with the message in its box.
  bool prefilled = false;

  @override
  bool prefills(PreparedHandoff handoff, AiApp? app) => prefilled;

  @override
  Future<String> handoffDirectory() async => '/cache/shared/handoff';

  @override
  Future<List<AiApp>> installedApps() async => apps;

  @override
  Future<void> share(PreparedHandoff handoff, AiApp? app) async =>
      calls.add('share ${app?.name ?? 'sheet'} ${handoff.files.length}');

  @override
  Future<String?> readClipboard() async => clipboard;

  @override
  Future<void> writeClipboard(String text) async => clipboard = text;
}

void main() {
  late FakeHandoffPorts ports;
  late DateTime clock;
  late List<String> refreshed;
  late HandoffController controller;

  setUp(() {
    ports = FakeHandoffPorts();
    clock = DateTime.utc(2026, 11, 12, 20);
    refreshed = <String>[];
    controller = HandoffController(
      core: ports,
      platform: ports,
      now: () => clock,
      onSummaryChanged: refreshed.add,
    );
  });

  final String answer = ('Decisions: ship on Friday. ' * 5).trim();

  test('a summary hand-off shares the files and keeps the copied answer',
      () async {
    final List<HandoffEvent> events = <HandoffEvent>[];
    controller.events.listen(events.add);

    expect(
        await controller.handOff('m-1', 'Review', HandoffPurpose.summary,
            app: AiApp.claude),
        isNull);
    expect(ports.calls, <String>[
      'prepare m-1 summary /cache/shared/handoff',
      'share claude 1',
    ]);
    expect(ports.clipboard, 'Summarize it.',
        reason: 'the prompt is also on the clipboard');

    // Back without copying anything: nothing is kept, and the owner is told how.
    await controller.checkClipboard();
    await pumpEventQueue();
    expect(
        ports.calls.where((String call) => call.startsWith('keep')), isEmpty);
    expect(events.single, isA<HandoffHint>());

    ports.clipboard = answer;
    await controller.checkClipboard();
    await pumpEventQueue();
    expect(ports.calls.last,
        'keep m-1 Claude app "Summarize it." ${answer.length}');
    expect(refreshed, <String>['m-1']);
    final HandoffKept kept = events.last as HandoffKept;
    expect(kept.title, 'Review');
    expect(kept.appName, 'Claude');
    expect(controller.pending, isNull);

    // Coming back again does not keep it twice.
    await controller.checkClipboard();
    expect(ports.calls.where((String call) => call.startsWith('keep')),
        hasLength(1));
  });

  test('Undo puts back the summary the answer replaced', () async {
    ports.previousSummary = <String, Object?>{'text': 'Earlier'};
    final List<HandoffEvent> events = <HandoffEvent>[];
    controller.events.listen(events.add);
    await controller.handOff('m-1', 'Review', HandoffPurpose.summary);
    ports.clipboard = answer;
    await controller.checkClipboard();
    await pumpEventQueue();

    expect(await controller.undo(events.last as HandoffKept), isNull);
    expect(ports.calls.last, 'restore m-1 Earlier');
    expect(refreshed, <String>['m-1', 'm-1']);
  });

  test('a chat hand-off leaves nothing to keep', () async {
    await controller.handOff('m-1', 'Review', HandoffPurpose.chat,
        app: AiApp.claude);
    expect(ports.calls.last, 'share claude 1');
    expect(controller.pending, isNull);
    ports.clipboard = answer;
    await controller.checkClipboard();
    expect(
        ports.calls.where((String call) => call.startsWith('keep')), isEmpty);
  });

  test("a project's chat goes with its files and leaves nothing to keep (#126)",
      () async {
    ports.viaBrowser = true;
    final List<HandoffSteps> shown = <HandoffSteps>[];

    final String? problem = await controller.handOffProject('p-1',
        app: AiApp.chatgpt, confirm: (HandoffSteps steps) async {
      shown.add(steps);
      return true;
    });

    expect(problem, isNull);
    expect(ports.calls, <String>[
      'prepare project p-1 /cache/shared/handoff',
      'share chatgpt 2',
    ]);
    expect(shown.single.purpose, HandoffPurpose.chat);
    expect(shown.single.hasFiles, isTrue);
    expect(controller.pending, isNull);
    expect(ports.clipboard, 'Read it.');
  });

  testWidgets('the chat sheet offers only chats, for a meeting or a project',
      (WidgetTester tester) async {
    ports.apps = <AiApp>[AiApp.chatgpt];
    await controller.loadApps();
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (BuildContext context) => Column(children: <Widget>[
            TextButton(
              onPressed: () => showChatHandoffSheet(context,
                  controller: controller, projectId: 'p-1'),
              child: const Text('Project'),
            ),
            TextButton(
              onPressed: () => showChatHandoffSheet(context,
                  controller: controller, meetingId: 'm-1'),
              child: const Text('Meeting'),
            ),
          ]),
        ),
      ),
    ));

    await tester.tap(find.text('Project'));
    await tester.pumpAndSettle();
    expect(find.text('Summarize in ChatGPT'), findsNothing);
    await tester.tap(find.text('Chat in ChatGPT'));
    await tester.pumpAndSettle();
    expect(ports.calls.first, 'prepare project p-1 /cache/shared/handoff');

    await tester.tap(find.text('Meeting'));
    await tester.pumpAndSettle();
    await tester.tap(find.text('Chat in ChatGPT'));
    await tester.pumpAndSettle();
    expect(ports.calls, contains('prepare m-1 chat /cache/shared/handoff'));
  });

  testWidgets("a meeting's or a project's chat page offers the own AI app",
      (WidgetTester tester) async {
    Future<void> show(ChatTopic topic, HandoffController? handoff) async {
      await tester.pumpWidget(MaterialApp(
        home: ChatScreen(
          bridge: ChatBridge(
              call: (String function, List<String?> arguments) async =>
                  '{"ok":true,"messages":[]}'),
          topic: topic,
          title: 'Chat',
          handoff: handoff,
        ),
      ));
      await tester.pumpAndSettle();
    }

    final Finder action = find.byKey(const ValueKey<String>('chat-handoff'));
    await show(const ChatTopic.project('p-1'), controller);
    expect(action, findsOneWidget);
    await show(const ChatTopic.meeting('m-1'), controller);
    expect(action, findsOneWidget);
    await show(const ChatTopic.everything(), controller);
    expect(action, findsNothing, reason: 'every meeting is too much to send');
    await show(const ChatTopic.meeting('m-1'), null);
    expect(action, findsNothing);
  });

  test('a short copy or one long after the hand-off is not kept', () async {
    await controller.handOff('m-1', 'Review', HandoffPurpose.summary);
    ports.clipboard = 'ok';
    await controller.checkClipboard();
    expect(
        ports.calls.where((String call) => call.startsWith('keep')), isEmpty);
    expect(controller.pending, isNotNull);

    clock = clock.add(const Duration(hours: 2));
    ports.clipboard = answer;
    await controller.checkClipboard();
    expect(
        ports.calls.where((String call) => call.startsWith('keep')), isEmpty);
    expect(controller.pending, isNull, reason: 'an old hand-off is forgotten');
  });

  test('a meeting with nothing to hand off is worded', () async {
    ports.failure = 'nothing_to_summarize';
    expect(await controller.handOff('m-1', 'Review', HandoffPurpose.summary),
        'This meeting has no transcript or notes yet.');
    expect(controller.pending, isNull);
  });

  test('only installed apps are offered', () async {
    ports.apps = <AiApp>[AiApp.chatgpt];
    await controller.loadApps();
    expect(controller.apps, <AiApp>[AiApp.chatgpt]);
  });

  testWidgets(
      'the sheet offers the installed app, and a kept answer can be undone',
      (WidgetTester tester) async {
    ports.apps = <AiApp>[AiApp.chatgpt];
    await controller.loadApps();
    await tester.pumpWidget(MaterialApp(
      home: HandoffReturnListener(
        controller: controller,
        child: Scaffold(
          body: Builder(
            builder: (BuildContext context) => TextButton(
              onPressed: () => showHandoffSheet(context,
                  controller: controller, meetingId: 'm-1', title: 'Review'),
              child: const Text('Own app'),
            ),
          ),
        ),
      ),
    ));
    await tester.tap(find.text('Own app'));
    await tester.pumpAndSettle();
    expect(find.text('Summarize in ChatGPT'), findsOneWidget);
    expect(find.text('Summarize in Claude'), findsNothing);

    await tester.tap(find.text('Summarize in ChatGPT'));
    await tester.pumpAndSettle();
    expect(ports.calls.last, 'share chatgpt 1');

    ports.clipboard = answer;
    await tester.runAsync(controller.checkClipboard);
    await tester.pumpAndSettle();
    expect(
        find.text('Summary from ChatGPT saved to "Review".'), findsOneWidget);
    await tester.tap(find.text('Undo'));
    await tester.pumpAndSettle();
    expect(ports.calls.last, 'restore m-1 null');
  });

  test('on a computer the steps are shown first, and Cancel shares nothing',
      () async {
    ports.viaBrowser = true;
    ports.prefilled = true;
    final List<HandoffSteps> shown = <HandoffSteps>[];

    final String? cancelled = await controller
        .handOff('m-1', 'Review', HandoffPurpose.summary, app: AiApp.claude,
            confirm: (HandoffSteps steps) async {
      shown.add(steps);
      return false;
    });

    expect(cancelled, isNull);
    expect(shown.single.prefilled, isTrue);
    expect(shown.single.app, AiApp.claude);
    expect(shown.single.hasFiles, isFalse);
    expect(
        ports.calls.where((String call) => call.startsWith('share')), isEmpty);
    expect(controller.pending, isNull);

    await controller.handOff('m-1', 'Review', HandoffPurpose.summary,
        app: AiApp.claude, confirm: (HandoffSteps steps) async => true);
    expect(ports.calls.last, 'share claude 1');
    expect(controller.pending, isNotNull);
  });

  test('on a computer the answer is credited to the website', () async {
    ports.viaBrowser = true;
    await controller.handOff('m-1', 'Review', HandoffPurpose.summary,
        app: AiApp.chatgpt);

    ports.clipboard = answer;
    await controller.checkClipboard();

    expect(ports.calls.last, startsWith('keep m-1 chatgpt.com '));
  });

  testWidgets('the steps say what to do in the website',
      (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: Builder(
          builder: (BuildContext context) => Column(children: <Widget>[
            TextButton(
              onPressed: () => showHandoffSteps(
                  context,
                  const HandoffSteps(
                      app: AiApp.claude,
                      purpose: HandoffPurpose.summary,
                      prefilled: true,
                      hasFiles: false)),
              child: const Text('prefilled'),
            ),
            TextButton(
              onPressed: () => showHandoffSteps(
                  context,
                  const HandoffSteps(
                      app: AiApp.chatgpt,
                      purpose: HandoffPurpose.summary,
                      prefilled: false,
                      hasFiles: true)),
              child: const Text('pasted'),
            ),
          ]),
        ),
      ),
    ));

    await tester.tap(find.text('prefilled'));
    await tester.pumpAndSettle();
    expect(find.textContaining('Press Enter'), findsOneWidget);
    expect(find.textContaining('Ctrl+V'), findsNothing);
    expect(find.textContaining('copy button'), findsOneWidget);
    await tester.tap(find.text('Cancel'));
    await tester.pumpAndSettle();

    await tester.tap(find.text('pasted'));
    await tester.pumpAndSettle();
    // The files go in before the message is sent.
    final double dragAt =
        tester.getTopLeft(find.textContaining('drag them')).dy;
    final double pasteAt = tester.getTopLeft(find.textContaining('Ctrl+V')).dy;
    expect(dragAt, lessThan(pasteAt));
    expect(find.text('Open chatgpt.com'), findsOneWidget);
  });

  test('an answer shared back as text is kept while a summary waits', () async {
    final List<HandoffEvent> events = <HandoffEvent>[];
    controller.events.listen(events.add);
    expect(await controller.keepSharedAnswer(answer), isFalse,
        reason: 'nothing is waiting');

    await controller.handOff('m-1', 'Review', HandoffPurpose.summary,
        app: AiApp.claude);
    expect(await controller.keepSharedAnswer('ok'), isFalse,
        reason: 'too short for a summary');
    expect(await controller.keepSharedAnswer(answer), isTrue);
    await pumpEventQueue();
    expect(ports.calls.last,
        'keep m-1 Claude app "Summarize it." ${answer.length}');
    expect(events.last, isA<HandoffKept>());
    expect(controller.pending, isNull);
  });

  testWidgets('a text share offers to save it as the waiting summary',
      (WidgetTester tester) async {
    await controller.handOff('m-1', 'Review', HandoffPurpose.summary,
        app: AiApp.claude);
    final _TextShareBridge shares = _TextShareBridge(IncomingShare(
      directory: '/cache/incoming/share-1',
      filePaths: const <String>['/cache/incoming/share-1/Shared text.txt'],
      failedCount: 0,
      sharedText: answer,
    ));
    final FakeProjectRunner runner =
        FakeProjectRunner((Map<String, Object?> request) => <String, Object?>{
              'ok': true,
              'projects': <Object?>[],
            });
    await tester.pumpWidget(MaterialApp(
      home: IncomingShareListener(
        shareBridge: shares,
        projectsController: ProjectsController(
          projectBridge: ProjectBridge(runCommand: runner.call),
          meetingBridge: FakeMeetingBridge(meetings: const <MeetingSummary>[]),
          filesPicker: const _NoPicker(),
        ),
        handoffController: controller,
        child: HandoffReturnListener(
            controller: controller, child: const Scaffold()),
      ),
    ));
    await tester.pumpAndSettle();

    expect(find.text('Save as the summary of "Review"'), findsOneWidget);
    expect(find.text('Add to a project instead'), findsOneWidget);
    await tester.tap(find.text('Save as the summary of "Review"'));
    await tester.pumpAndSettle();

    expect(ports.calls.last,
        'keep m-1 Claude app "Summarize it." ${answer.length}');
    expect(find.text('Summary from Claude saved to "Review".'), findsOneWidget);
    expect(shares.released, <String>['/cache/incoming/share-1']);
  });
}
