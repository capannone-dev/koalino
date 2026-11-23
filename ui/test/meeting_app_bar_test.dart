// Specifies where the meeting screen's actions go: beside the title on wide screens, and on a row
// of their own on phones, where up to eight of them would slide over the back button and the
// title (seen on the Android emulator: Back and Chat overlapped).

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'meeting_attachments_test.dart' show pumpDetail;

/// Sets the test window to `width` logical pixels (density 1) for the rest of the test.
void _useWidth(WidgetTester tester, double width) {
  tester.view.physicalSize = Size(width, 800);
  tester.view.devicePixelRatio = 1;
  addTearDown(tester.view.reset);
}

void main() {
  testWidgets('on a phone the actions sit on their own row under the title',
      (WidgetTester tester) async {
    _useWidth(tester, 360);
    await pumpDetail(tester);

    final Rect title = tester.getRect(find.text('Meeting'));
    final Rect attach = tester.getRect(find.byTooltip('Attach files'));
    expect(attach.top, greaterThanOrEqualTo(title.bottom));
    expect(tester.takeException(), isNull);
  });

  testWidgets('on a wide screen the actions stay beside the title',
      (WidgetTester tester) async {
    _useWidth(tester, 1000);
    await pumpDetail(tester);

    final Rect title = tester.getRect(find.text('Meeting'));
    final Rect attach = tester.getRect(find.byTooltip('Attach files'));
    expect(attach.center.dy, closeTo(title.center.dy, 4));
  });
}
