// Defines widget-first requirements for the responsive, title-first meeting list.

import 'dart:convert';

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';
import 'package:flutter/rendering.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/meeting_time.dart';
import 'package:note_taker_ui/controllers/meeting_edit_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/controllers/summaries_controller.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/meeting_edit_bridge.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';
import 'package:note_taker_ui/presentation/rename_meeting_dialog.dart';
import 'package:note_taker_ui/presentation/resizable_pane.dart';

import 'fake_llm_core.dart';
import 'fake_meeting_bridge.dart';

/// Pumps the meeting list at `size` (logical pixels) around `meetingBridge`.
Future<void> pumpMeetingList(
    WidgetTester tester, FakeMeetingBridge meetingBridge,
    {Size size = const Size(390, 844),
    MeetingScreenBuilder? speakersScreenBuilder,
    SummariesController? summariesController,
    MeetingEditController? editController}) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = size;
  addTearDown(tester.view.reset);
  await tester.pumpWidget(
    MaterialApp(
      home: MeetingListScreen(
        controller: MeetingListController(meetingBridge: meetingBridge),
        speakersScreenBuilder: speakersScreenBuilder,
        summariesController: summariesController,
        editController: editController,
      ),
    ),
  );
  await tester.pumpAndSettle();
}

/// Verifies titles, metadata, states, and the phone and desktop layouts of the meeting list.
void main() {
  testWidgets('shows custom and fallback titles with their date and duration',
      (WidgetTester tester) async {
    await pumpMeetingList(
      tester,
      FakeMeetingBridge(
          meetings: <MeetingSummary>[titledMeeting, untitledMeeting]),
    );

    expect(find.text('Project Sync'), findsOneWidget);
    expect(find.text('Meeting 2026-10-26 18:00 UTC'), findsOneWidget);
    expect(
        find.text(
            '${formatMeetingWhen(DateTime.utc(2026, 10, 27, 9, 5))} · 1:05'),
        findsOneWidget);
    expect(
        find.text(
            '${formatMeetingWhen(DateTime.utc(2026, 10, 26, 18, 0))} · 0:00'),
        findsOneWidget);
  });

  testWidgets('places the title above, and larger than, its metadata',
      (WidgetTester tester) async {
    await pumpMeetingList(
        tester, FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting]));

    final Finder titleText = find.text('Project Sync');
    final Finder metadataText = find
        .text('${formatMeetingWhen(DateTime.utc(2026, 10, 27, 9, 5))} · 1:05');
    expect(tester.getTopLeft(titleText).dy,
        lessThan(tester.getTopLeft(metadataText).dy));
    final TextStyle titleStyle =
        tester.renderObject<RenderParagraph>(titleText).text.style!;
    final TextStyle metadataStyle =
        tester.renderObject<RenderParagraph>(metadataText).text.style!;
    expect(titleStyle.fontSize!, greaterThan(metadataStyle.fontSize!));
    expect(titleStyle.fontWeight, FontWeight.w600);
  });

  testWidgets('tapping a meeting on a phone opens its detail screen',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
      meetings: <MeetingSummary>[titledMeeting],
      details: <String, MeetingDetail>{'meeting-titled': titledMeetingDetail},
    );
    await pumpMeetingList(tester, meetingBridge);

    await tester.tap(find.text('Project Sync'));
    await tester.pumpAndSettle();

    expect(find.byType(MeetingDetailScreen), findsOneWidget);
    expect(meetingBridge.requestedMeetings, <String>['meeting-titled']);
    expect(find.text('Hello team.'), findsOneWidget);
  });

  group('Name speakers', () {
    /// Stands in for the voice gallery, naming the meeting it was opened for.
    Widget fakeSpeakersScreen(BuildContext context, String meetingId) =>
        Scaffold(appBar: AppBar(), body: Text('Speakers of $meetingId'));

    /// A bridge with one meeting and its detail.
    FakeMeetingBridge oneMeetingBridge() => FakeMeetingBridge(
          meetings: <MeetingSummary>[titledMeeting],
          details: <String, MeetingDetail>{
            'meeting-titled': titledMeetingDetail
          },
        );

    testWidgets('a phone opens the speakers of the shown meeting',
        (WidgetTester tester) async {
      await pumpMeetingList(tester, oneMeetingBridge(),
          speakersScreenBuilder: fakeSpeakersScreen);
      await tester.tap(find.text('Project Sync'));
      await tester.pumpAndSettle();

      await tester.tap(find.byTooltip('Name speakers'));
      await tester.pumpAndSettle();

      expect(find.text('Speakers of meeting-titled'), findsOneWidget);
    });

    testWidgets('coming back reloads the meeting, so new names show',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = oneMeetingBridge();
      await pumpMeetingList(tester, meetingBridge,
          speakersScreenBuilder: fakeSpeakersScreen);
      await tester.tap(find.text('Project Sync'));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Name speakers'));
      await tester.pumpAndSettle();

      await tester.pageBack();
      await tester.pumpAndSettle();

      expect(meetingBridge.requestedMeetings,
          <String>['meeting-titled', 'meeting-titled']);
      expect(find.text('Hello team.'), findsOneWidget);
    });

    testWidgets('a desktop window offers it once a meeting is selected',
        (WidgetTester tester) async {
      await pumpMeetingList(tester, oneMeetingBridge(),
          size: const Size(1280, 800),
          speakersScreenBuilder: fakeSpeakersScreen);
      expect(find.byTooltip('Name speakers'), findsNothing);

      await tester.tap(find.text('Project Sync'));
      await tester.pumpAndSettle();
      await tester.tap(find.byTooltip('Name speakers'));
      await tester.pumpAndSettle();

      expect(find.text('Speakers of meeting-titled'), findsOneWidget);
    });

    testWidgets('is hidden without a speakers screen',
        (WidgetTester tester) async {
      await pumpMeetingList(tester, oneMeetingBridge());
      await tester.tap(find.text('Project Sync'));
      await tester.pumpAndSettle();

      expect(find.byTooltip('Name speakers'), findsNothing);
    });
  });

  testWidgets('a desktop window shows the list and the detail side by side',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
      meetings: <MeetingSummary>[titledMeeting, untitledMeeting],
      details: <String, MeetingDetail>{'meeting-titled': titledMeetingDetail},
    );
    await pumpMeetingList(tester, meetingBridge, size: const Size(1280, 800));

    expect(
        find.text('Select a meeting to see its transcript.'), findsOneWidget);

    await tester.tap(find.text('Project Sync'));
    await tester.pumpAndSettle();

    expect(find.byType(MeetingDetailScreen), findsNothing);
    expect(find.text('Hello team.'), findsOneWidget);
    expect(find.text('Project Sync'), findsNWidgets(2));
    final ListTile selectedTile = tester.widget<ListTile>(find.ancestor(
        of: find.text('Meeting 2026-10-26 18:00 UTC'),
        matching: find.byType(ListTile)));
    expect(selectedTile.selected, isFalse);
  });

  testWidgets(
      'the mouse wheel scrolls the note anywhere across its pane, not only over the text (#142)',
      (WidgetTester tester) async {
    final MeetingDetail longMeeting = MeetingDetail(
      summary: titledMeeting,
      transcript: null,
      segments: <MeetingTranscriptSegment>[
        for (int index = 0; index < 80; index++)
          MeetingTranscriptSegment(
              startTimeMs: index * 5000,
              endTimeMs: index * 5000 + 4000,
              speaker: 'Alice',
              text: 'Line $index of a long meeting.'),
      ],
    );
    await pumpMeetingList(
        tester,
        FakeMeetingBridge(
          meetings: <MeetingSummary>[titledMeeting],
          details: <String, MeetingDetail>{'meeting-titled': longMeeting},
        ),
        size: const Size(1800, 800));
    await tester.tap(find.text('Project Sync'));
    await tester.pumpAndSettle();
    final Finder note = find.ancestor(
        of: find.text('Line 0 of a long meeting.'),
        matching: find.byType(Scrollable));
    expect(tester.getTopLeft(note.first).dx, lessThan(500),
        reason: 'the scrolling area spans the pane, not only the text column');

    final ScrollableState scrollable =
        tester.state<ScrollableState>(note.first);
    final TestPointer mouse = TestPointer(1, PointerDeviceKind.mouse);
    mouse.hover(const Offset(1790, 500));
    await tester.sendEventToBinding(mouse.scroll(const Offset(0, 400)));
    await tester.pumpAndSettle();

    expect(scrollable.position.pixels, greaterThan(0));
  });

  testWidgets(
      'dragging the divider widens the meeting list, and the width is remembered (#142)',
      (WidgetTester tester) async {
    final MemoryPaneWidthStore store = MemoryPaneWidthStore();
    paneWidthStore = store;
    addTearDown(() => paneWidthStore = MemoryPaneWidthStore());
    final FakeMeetingBridge bridge =
        FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting]);
    await pumpMeetingList(tester, bridge, size: const Size(1280, 800));
    final Finder pane = find.byKey(const ValueKey<String>('pane-meeting-list'));
    expect(tester.getSize(pane).width, 360);

    await tester.drag(find.byKey(const ValueKey<String>('resize-meeting-list')),
        const Offset(120, 0));
    await tester.pumpAndSettle();

    expect(tester.getSize(pane).width, closeTo(480, 1));
    expect(store.widths['meeting-list'], closeTo(480, 1));

    await tester.pumpWidget(const SizedBox());
    await pumpMeetingList(tester, bridge, size: const Size(1280, 800));
    expect(tester.getSize(pane).width, closeTo(480, 1));

    await tester.drag(find.byKey(const ValueKey<String>('resize-meeting-list')),
        const Offset(5000, 0));
    await tester.pumpAndSettle();
    expect(tester.getSize(pane).width, lessThanOrEqualTo(1280 - 400),
        reason: 'the note keeps at least 400 px');
  });

  group('deleting a meeting', () {
    testWidgets('asks first, and Cancel keeps it', (WidgetTester tester) async {
      final FakeMeetingBridge bridge =
          FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting]);
      await pumpMeetingList(tester, bridge);

      await tester
          .tap(find.byKey(const ValueKey<String>('delete-meeting-titled')));
      await tester.pumpAndSettle();
      expect(find.text('Delete Project Sync?'), findsOneWidget);
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();

      expect(bridge.deleteRequests, isEmpty);
      expect(find.text('Project Sync'), findsOneWidget);
    });

    testWidgets('from the list, removes only that row',
        (WidgetTester tester) async {
      final FakeMeetingBridge bridge = FakeMeetingBridge(
          meetings: <MeetingSummary>[titledMeeting, untitledMeeting]);
      await pumpMeetingList(tester, bridge);

      await tester
          .tap(find.byKey(const ValueKey<String>('delete-meeting-titled')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();

      expect(bridge.deleteRequests, <String>['meeting-titled']);
      expect(find.text('Project Sync'), findsNothing);
      expect(find.text('Meeting 2026-10-26 18:00 UTC'), findsOneWidget);
    });

    testWidgets('from an open meeting on a phone, returns to the list',
        (WidgetTester tester) async {
      final FakeMeetingBridge bridge = FakeMeetingBridge(
        meetings: <MeetingSummary>[titledMeeting],
        details: <String, MeetingDetail>{
          titledMeeting.meetingId: MeetingDetail(
            summary: titledMeeting,
            transcript: '',
            segments: const <MeetingTranscriptSegment>[],
          ),
        },
      );
      await pumpMeetingList(tester, bridge);
      await tester.tap(find.text('Project Sync'));
      await tester.pumpAndSettle();

      await tester
          .tap(find.byKey(const ValueKey<String>('delete-open-meeting')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete'));
      await tester.pumpAndSettle();

      expect(bridge.deleteRequests, <String>['meeting-titled']);
      expect(find.byType(MeetingDetailScreen), findsNothing);
      expect(find.text('No meetings yet'), findsOneWidget);
    });

    testWidgets('a refusal keeps it and explains', (WidgetTester tester) async {
      final FakeMeetingBridge bridge =
          FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting])
            ..deleteError = StateError('recording');
      await pumpMeetingList(tester, bridge);

      await tester
          .tap(find.byKey(const ValueKey<String>('delete-meeting-titled')));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Delete'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 100));

      expect(find.text('Project Sync'), findsOneWidget);
      expect(
          find.textContaining('Unable to delete this meeting'), findsOneWidget);
    });
  });

  group('a new meeting without a recording (#129)', () {
    testWidgets('asks for a title, creates it and opens it',
        (WidgetTester tester) async {
      final FakeLlmCore core = FakeLlmCore();
      core.answer('edit:create_meeting',
          <String, Object?>{'ok': true, 'meeting_id': 'meeting-new'});
      core.answer('edit:notes', <String, Object?>{'ok': true, 'notes': null});
      await pumpMeetingList(
        tester,
        FakeMeetingBridge(details: <String, MeetingDetail>{
          'meeting-new': MeetingDetail(
            summary: titledMeeting,
            transcript: null,
            segments: const <MeetingTranscriptSegment>[],
          ),
        }),
        editController:
            MeetingEditController(bridge: MeetingEditBridge(call: core.call)),
      );

      await tester.tap(find.byKey(const ValueKey<String>('new-meeting')));
      await tester.pumpAndSettle();
      expect(find.text('New meeting'), findsWidgets);
      await tester.enterText(find.byType(TextField), 'Supplier call');
      await tester.tap(find.text('Create'));
      await tester.pumpAndSettle();

      final Map<String, Object?> request = core.calls
          .where(((String, List<String?>) call) =>
              call.$1 == 'note_taker_meeting_edit')
          .map(((String, List<String?>) call) =>
              jsonDecode(call.$2.single!) as Map<String, Object?>)
          .first;
      expect(request['command'], 'create_meeting');
      expect(request['title'], 'Supplier call');
      expect(find.byType(MeetingDetailScreen), findsOneWidget);
    });

    testWidgets('a meeting with no recording says what to add',
        (WidgetTester tester) async {
      final MeetingSummary written = MeetingSummary(
        meetingId: 'meeting-written',
        title: 'Supplier call',
        createdAt: DateTime.utc(2026, 11, 15, 16, 17),
        durationMs: 0,
      );
      await pumpMeetingList(
        tester,
        FakeMeetingBridge(
          meetings: <MeetingSummary>[written],
          details: <String, MeetingDetail>{
            'meeting-written': MeetingDetail(
              summary: written,
              transcript: null,
              segments: const <MeetingTranscriptSegment>[],
            ),
          },
        ),
      );

      await tester.tap(find.text('Supplier call'));
      await tester.pumpAndSettle();

      expect(find.textContaining('No recording. Write notes'), findsOneWidget);
    });

    testWidgets('is offered only when meetings can be edited',
        (WidgetTester tester) async {
      await pumpMeetingList(tester, FakeMeetingBridge());

      expect(find.byKey(const ValueKey<String>('new-meeting')), findsNothing);
    });
  });

  testWidgets('explains when there are no meetings yet',
      (WidgetTester tester) async {
    await pumpMeetingList(tester, FakeMeetingBridge());

    expect(find.text('No meetings yet'), findsOneWidget);
    expect(
        find.widgetWithText(FilledButton, 'Record a meeting'), findsOneWidget,
        reason: 'the empty list offers the next step (#204)');
  });

  testWidgets('shows an error with a retry action when loading fails',
      (WidgetTester tester) async {
    final FakeMeetingBridge meetingBridge =
        FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting])
          ..listError = StateError('disk unavailable');
    await pumpMeetingList(tester, meetingBridge);

    expect(find.text('Unable to load meetings'), findsOneWidget);

    meetingBridge.listError = null;
    await tester.tap(find.text('Retry'));
    await tester.pumpAndSettle();

    expect(find.text('Project Sync'), findsOneWidget);
  });

  group('rename', () {
    /// Pumps the list with both fixture meetings and returns its fake bridge.
    Future<FakeMeetingBridge> pumpRenamableList(WidgetTester tester,
        {Size size = const Size(390, 844)}) async {
      final FakeMeetingBridge meetingBridge = FakeMeetingBridge(
        meetings: <MeetingSummary>[titledMeeting, untitledMeeting],
        details: <String, MeetingDetail>{'meeting-titled': titledMeetingDetail},
      );
      await pumpMeetingList(tester, meetingBridge, size: size);
      return meetingBridge;
    }

    /// Opens the rename dialog for "Project Sync" through its long-press menu.
    Future<void> openRenameDialog(WidgetTester tester) async {
      await tester.longPress(find.text('Project Sync'));
      await tester.pumpAndSettle();
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
    }

    /// Replaces the dialog's title text and presses Save.
    Future<void> submitTitle(WidgetTester tester, String newTitle) async {
      await tester.enterText(
          find.descendant(
              of: find.byType(RenameMeetingDialog),
              matching: find.byType(TextField)),
          newTitle);
      await tester.tap(find.text('Save'));
      await tester.pumpAndSettle();
    }

    testWidgets('long-pressing a meeting shows a Rename option',
        (WidgetTester tester) async {
      await pumpRenamableList(tester);

      await tester.longPress(find.text('Project Sync'));
      await tester.pumpAndSettle();

      expect(find.text('Rename'), findsOneWidget);
    });

    testWidgets('right-clicking a meeting shows a Rename option',
        (WidgetTester tester) async {
      await pumpRenamableList(tester, size: const Size(1280, 800));

      await tester.tap(find.text('Project Sync'), buttons: kSecondaryButton);
      await tester.pumpAndSettle();

      expect(find.text('Rename'), findsOneWidget);
    });

    testWidgets('Shift+F10 on a focused meeting shows a Rename option',
        (WidgetTester tester) async {
      await pumpRenamableList(tester, size: const Size(1280, 800));
      Focus.of(tester.element(find.text('Project Sync'))).requestFocus();
      await tester.pump();

      await tester.sendKeyDownEvent(LogicalKeyboardKey.shiftLeft);
      await tester.sendKeyEvent(LogicalKeyboardKey.f10);
      await tester.sendKeyUpEvent(LogicalKeyboardKey.shiftLeft);
      await tester.pumpAndSettle();

      expect(find.text('Rename'), findsOneWidget);
    });

    testWidgets('screen readers get a Rename action on each meeting',
        (WidgetTester tester) async {
      final SemanticsHandle semantics = tester.ensureSemantics();
      await pumpRenamableList(tester);

      final int renameActionId = CustomSemanticsAction.getIdentifier(
          const CustomSemanticsAction(label: 'Rename'));
      SemanticsNode? semanticsNode =
          tester.getSemantics(find.text('Project Sync'));
      bool exposesRename = false;
      while (semanticsNode != null && !exposesRename) {
        exposesRename = semanticsNode
                .getSemanticsData()
                .customSemanticsActionIds
                ?.contains(renameActionId) ??
            false;
        semanticsNode = semanticsNode.parent;
      }
      expect(exposesRename, isTrue);
      semantics.dispose();
    });

    testWidgets('Rename opens a dialog pre-filled with the current title',
        (WidgetTester tester) async {
      await pumpRenamableList(tester);

      await openRenameDialog(tester);

      expect(find.text('Rename meeting'), findsOneWidget);
      final TextField titleField = tester.widget<TextField>(find.descendant(
          of: find.byType(RenameMeetingDialog),
          matching: find.byType(TextField)));
      expect(titleField.controller?.text, 'Project Sync');
      expect(titleField.controller?.selection,
          const TextSelection(baseOffset: 0, extentOffset: 12));
      expect(find.text('Cancel'), findsOneWidget);
      expect(find.text('Save'), findsOneWidget);
    });

    testWidgets('an empty title shows an error and is not sent',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);

      await submitTitle(tester, '   ');

      expect(find.text('Enter a title.'), findsOneWidget);
      expect(find.byType(RenameMeetingDialog), findsOneWidget);
      expect(meetingBridge.renameRequests, isEmpty);
    });

    testWidgets('a title over 200 characters shows an error and is not sent',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);

      await submitTitle(tester, 'a' * 201);

      expect(find.text('Titles can be at most 200 characters (you have 201).'),
          findsOneWidget);
      expect(meetingBridge.renameRequests, isEmpty);
    });

    testWidgets('a control character shows an error and is not sent',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);

      await submitTitle(tester, 'Weekly\tsync');

      expect(
          find.text("Titles can't contain line breaks or control characters."),
          findsOneWidget);
      expect(meetingBridge.renameRequests, isEmpty);
    });

    testWidgets('a successful rename closes the dialog and updates the row',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);

      await submitTitle(tester, '  Roadmap Review  ');

      expect(meetingBridge.renameRequests, <RenameRequest>[
        (meetingId: 'meeting-titled', newTitle: 'Roadmap Review')
      ]);
      expect(find.byType(RenameMeetingDialog), findsNothing);
      expect(find.text('Roadmap Review'), findsOneWidget);
      expect(find.text('Project Sync'), findsNothing);
      expect(find.text('Meeting renamed'), findsOneWidget);
      expect(meetingBridge.meetingListRequestCount, 1);
    });

    testWidgets('pressing Enter in the field saves the title',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);

      await tester.enterText(find.byType(TextField), 'Roadmap Review');
      await tester.testTextInput.receiveAction(TextInputAction.done);
      await tester.pumpAndSettle();

      expect(meetingBridge.renameRequests, hasLength(1));
      expect(find.text('Roadmap Review'), findsOneWidget);
    });

    testWidgets('saving an unchanged title closes without calling Rust',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);

      await submitTitle(tester, ' Project Sync ');

      expect(find.byType(RenameMeetingDialog), findsNothing);
      expect(meetingBridge.renameRequests, isEmpty);
    });

    testWidgets('Cancel closes the dialog without renaming',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);
      await tester.enterText(find.byType(TextField), 'Roadmap Review');

      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();

      expect(find.byType(RenameMeetingDialog), findsNothing);
      expect(meetingBridge.renameRequests, isEmpty);
      expect(find.text('Project Sync'), findsOneWidget);
    });

    testWidgets('Escape closes the dialog without renaming',
        (WidgetTester tester) async {
      final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
      await openRenameDialog(tester);

      await tester.sendKeyEvent(LogicalKeyboardKey.escape);
      await tester.pumpAndSettle();

      expect(find.byType(RenameMeetingDialog), findsNothing);
      expect(meetingBridge.renameRequests, isEmpty);
    });

    for (final (int statusCode, String message) in <(int, String)>[
      (1, "This title isn't allowed."),
      (2, "Couldn't save the new title. Try again."),
      (3, "The note-taking engine isn't available right now."),
      (5, 'This meeting no longer exists.'),
      (42, "Couldn't rename the meeting."),
    ]) {
      testWidgets(
          'Rust status $statusCode keeps the dialog open with a message',
          (WidgetTester tester) async {
        final FakeMeetingBridge meetingBridge = await pumpRenamableList(tester);
        meetingBridge.renameError = meetingRenameExceptionFor(statusCode);
        await openRenameDialog(tester);

        await submitTitle(tester, 'Roadmap Review');

        expect(find.text(message), findsOneWidget);
        expect(find.byType(RenameMeetingDialog), findsOneWidget);
        expect(find.text('Meeting renamed'), findsNothing);
      });
    }

    testWidgets('on desktop a rename from the list updates the detail pane',
        (WidgetTester tester) async {
      await pumpRenamableList(tester, size: const Size(1280, 800));
      await tester.tap(find.text('Project Sync'));
      await tester.pumpAndSettle();
      expect(find.text('Project Sync'), findsNWidgets(2));

      await tester.tap(find.text('Project Sync').first,
          buttons: kSecondaryButton);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
      await submitTitle(tester, 'Roadmap Review');

      expect(find.text('Roadmap Review'), findsNWidgets(2));
      expect(find.text('Project Sync'), findsNothing);
    });

    /// Measures the dialog's visible surface; `AlertDialog` itself fills the screen.
    double dialogSurfaceWidth(WidgetTester tester) => tester
        .getSize(find
            .descendant(
                of: find.byType(AlertDialog), matching: find.byType(Material))
            .first)
        .width;

    testWidgets('the dialog stays compact on desktop and fits a phone',
        (WidgetTester tester) async {
      await pumpRenamableList(tester, size: const Size(1280, 800));
      await tester.tap(find.text('Project Sync'), buttons: kSecondaryButton);
      await tester.pumpAndSettle();
      await tester.tap(find.text('Rename'));
      await tester.pumpAndSettle();
      expect(dialogSurfaceWidth(tester), lessThanOrEqualTo(560));
      await tester.tap(find.text('Cancel'));
      await tester.pumpAndSettle();

      tester.view.physicalSize = const Size(320, 640);
      await tester.pumpAndSettle();
      await openRenameDialog(tester);
      expect(dialogSurfaceWidth(tester), lessThanOrEqualTo(320));
      expect(tester.takeException(), isNull);
    });
  });

  testWidgets('a combined meeting names its devices in the row (#88)',
      (WidgetTester tester) async {
    await pumpMeetingList(
      tester,
      FakeMeetingBridge(meetings: <MeetingSummary>[combinedMeeting]),
    );

    expect(
        find.text(
            '${formatMeetingWhen(DateTime.utc(2026, 11, 2, 15, 0))} · 2:00 · Laptop + Pixel'),
        findsOneWidget);
  });

  group('summaries (#95)', () {
    testWidgets('each row has a summary button that opens the summary',
        (WidgetTester tester) async {
      final FakeLlmCore core = FakeLlmCore()
        ..answer('command:settings', settings())
        ..answer('note_taker_get_meeting_summary', <String, Object?>{
          'ok': true,
          'summary': summary,
          'meeting_prompt': null,
        });
      await pumpMeetingList(
        tester,
        FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting]),
        summariesController:
            SummariesController(bridge: LlmBridge(call: core.call)),
      );

      await tester.tap(find.byTooltip('Summary of Project Sync'));
      await tester.pumpAndSettle();

      expect(find.text('Summary'), findsOneWidget);
      expect(find.text('Budget approved.'), findsOneWidget);
    });

    testWidgets('the row menu offers Summary… only with summaries',
        (WidgetTester tester) async {
      await pumpMeetingList(
          tester, FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting]));
      await tester.longPress(find.text('Project Sync'));
      await tester.pumpAndSettle();
      expect(find.text('Summary…'), findsNothing);
      expect(find.byTooltip('Summary of Project Sync'), findsNothing);
    });
  });
}
