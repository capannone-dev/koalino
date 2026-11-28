// Specifies where the meeting page's actions go (#201): in the page header, under the title, at
// most three of them (Ask AI, Share or Export, More), the same on phones and wide windows. Before,
// up to eight icons crowded the app bar and slid over the back button on phones.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/meeting_detail_screen.dart';

import 'meeting_attachments_test.dart' show pumpDetail;

/// Sets the test window to `width` logical pixels (density 1) for the rest of the test.
void _useWidth(WidgetTester tester, double width) {
  tester.view.physicalSize = Size(width, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
}

void main() {
  for (final double width in <double>[360, 1000]) {
    testWidgets(
        'at $width px the actions sit under the title, not in the app bar',
        (WidgetTester tester) async {
      _useWidth(tester, width);
      await pumpDetail(tester);

      final Rect title = tester.getRect(find.text('Project Sync'));
      final Rect more = tester.getRect(find.byKey(meetingMoreKey));
      expect(more.top, greaterThanOrEqualTo(title.bottom));
      expect(
          find.descendant(
              of: find.byType(AppBar), matching: find.byType(IconButton)),
          findsNothing,
          reason: 'the app bar keeps only Back and the page name');
      expect(tester.takeException(), isNull);
    });
  }
}
