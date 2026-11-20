// Specifies the phone's setting that lets the after-meeting pass run while Koalino is closed and
// Power saving is on (#87): Android asks once to exempt the app from battery optimisation.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/background_core.dart';
import 'package:note_taker_ui/l10n/l10n.dart';
import 'package:note_taker_ui/presentation/background_pass_tile.dart';

/// Answers like Android: exempt only after the owner allowed it.
final class _FakeBridge implements BackgroundPassBridge {
  bool exempt = false;
  int requests = 0;
  bool allowOnRequest = true;

  @override
  Future<bool?> isExempt() async => exempt;

  @override
  Future<void> requestExemption() async {
    requests++;
    if (allowOnRequest) {
      exempt = true;
    }
  }
}

Future<void> _pump(WidgetTester tester, BackgroundPassBridge bridge) async {
  await tester.pumpWidget(MaterialApp(
    home: Scaffold(body: BackgroundPassTile(bridge: bridge)),
  ));
  await tester.pumpAndSettle();
}

void main() {
  testWidgets('asks Android once, then shows the pass may run while closed',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, bridge);
    expect(find.text(l10n.backgroundPassAsk), findsOneWidget);

    await tester.tap(find.byKey(backgroundPassTileKey));
    await tester.pumpAndSettle();

    expect(bridge.requests, 1);
    expect(find.text(l10n.backgroundPassAllowed), findsOneWidget);
    await tester.tap(find.byKey(backgroundPassTileKey));
    await tester.pumpAndSettle();
    expect(bridge.requests, 1, reason: 'nothing to ask once allowed');
  });

  testWidgets('a refusal leaves the request on offer',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge()..allowOnRequest = false;
    await _pump(tester, bridge);

    await tester.tap(find.byKey(backgroundPassTileKey));
    await tester.pumpAndSettle();

    expect(find.text(l10n.backgroundPassAsk), findsOneWidget);
  });

  testWidgets('nothing is shown where the platform has no such setting',
      (WidgetTester tester) async {
    final _FakeBridge bridge = _FakeBridge();
    await _pump(tester, _UnsupportedBridge());
    expect(find.byKey(backgroundPassTileKey), findsNothing);
    expect(bridge.requests, 0);
  });
}

final class _UnsupportedBridge implements BackgroundPassBridge {
  @override
  Future<bool?> isExempt() async => null;

  @override
  Future<void> requestExemption() async {}
}
