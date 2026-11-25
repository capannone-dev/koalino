// Specifies Settings -> About Koalino (#115): the app's name, its license notice, and the
// licenses page of the packages it is built on, with the logo and the built version (#172).

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/about_tile.dart';
import 'package:note_taker_ui/presentation/koalino_logo.dart';

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
    expect(find.byKey(koalinoLogoKey), findsOneWidget);
  });

  test('About shows the version pubspec.yaml builds (#172)', () {
    final RegExpMatch? match = RegExp(r'^version: *([^+\s]+)', multiLine: true)
        .firstMatch(File('pubspec.yaml').readAsStringSync());
    expect(appVersion, match?.group(1));
  });
}
