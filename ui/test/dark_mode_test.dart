// Checks the main screens in the dark theme (#214): every text meets Flutter's contrast guideline,
// so nothing is left in a light-only colour.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/controllers/meeting_edit_controller.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/infrastructure/seen_tips_store.dart';
import 'package:note_taker_ui/presentation/app_theme.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';
import 'package:note_taker_ui/presentation/meeting_list_screen.dart';
import 'package:note_taker_ui/presentation/settings_screen.dart';

import 'fake_meeting_bridge.dart';
import 'transcription_route_test.dart' show newLogSettingsController;

/// Pumps `home` in the dark theme at phone size.
Future<void> pumpDark(WidgetTester tester, Widget home) async {
  tester.view.devicePixelRatio = 1;
  tester.view.physicalSize = const Size(390, 844);
  addTearDown(tester.view.reset);
  await tester.pumpWidget(MaterialApp(
    theme: noteTakerTheme(Brightness.light),
    darkTheme: noteTakerTheme(Brightness.dark),
    themeMode: ThemeMode.dark,
    home: home,
  ));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('the meeting list, empty and full', (WidgetTester tester) async {
    for (final FakeMeetingBridge bridge in <FakeMeetingBridge>[
      FakeMeetingBridge(),
      FakeMeetingBridge(meetings: <MeetingSummary>[titledMeeting]),
    ]) {
      await pumpDark(
          tester,
          MeetingListScreen(
              controller: MeetingListController(meetingBridge: bridge)));
      await expectLater(tester, meetsGuideline(textContrastGuideline));
    }
  });

  testWidgets('a meeting with its tip, tabs and unsure words',
      (WidgetTester tester) async {
    await pumpDark(
        tester,
        MeetingDetailScreen(
          controller: MeetingListController(
              meetingBridge: FakeMeetingBridge(details: <String, MeetingDetail>{
            'meeting-doubtful': doubtfulMeetingDetail
          })),
          meetingId: 'meeting-doubtful',
          editController:
              MeetingEditController(tipsStore: MemorySeenTipsStore()),
        ));
    await expectLater(tester, meetsGuideline(textContrastGuideline));
  });

  testWidgets('Settings and Advanced', (WidgetTester tester) async {
    await pumpDark(tester,
        SettingsScreen(logSettingsController: newLogSettingsController()));
    await expectLater(tester, meetsGuideline(textContrastGuideline));
    await tester.tap(find.byKey(advancedSettingsKey));
    await tester.pumpAndSettle();
    await expectLater(tester, meetsGuideline(textContrastGuideline));
  });
}
