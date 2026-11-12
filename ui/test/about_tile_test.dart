// Specifies Settings -> About Koalino (#115): the app's name, its license notice, and the
// licenses page of the packages it is built on.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/about_tile.dart';

void main() {
  testWidgets('About shows the name, the GPL notice and the licenses',
      (WidgetTester tester) async {
    await tester.pumpWidget(const MaterialApp(
      home: Scaffold(body: AboutKoalinoTile()),
    ));

    await tester.tap(find.text('About Koalino'));
    await tester.pumpAndSettle();

    expect(find.text('Koalino'), findsWidgets);
    expect(find.textContaining('GNU General Public License'), findsOneWidget);
    expect(find.textContaining('WITHOUT ANY WARRANTY'), findsOneWidget);
    expect(find.text('View licenses'), findsOneWidget);
  });
}
