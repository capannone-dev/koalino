// Specifies the power-user keys of the main screens (#215): Ctrl+K opens the command palette,
// R records, ? lists the shortcuts, and single keys stay text while typing in a field.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/keyboard_shortcuts.dart';

import 'home_shell_test.dart' show pumpHomeShell;

/// Presses a key that types `character`.
Future<void> typeKey(
    WidgetTester tester, LogicalKeyboardKey key, String character) async {
  await simulateKeyDownEvent(key, character: character);
  await simulateKeyUpEvent(key);
  await tester.pumpAndSettle();
}

void main() {
  const Size desktop = Size(1280, 800);

  testWidgets('R opens the recorder', (WidgetTester tester) async {
    await pumpHomeShell(tester, desktop);
    await typeKey(tester, LogicalKeyboardKey.keyR, 'r');
    expect(find.text('Recording screen'), findsOneWidget);
  });

  testWidgets('Ctrl+K opens the palette; typing filters and Enter runs',
      (WidgetTester tester) async {
    await pumpHomeShell(tester, desktop,
        withProjects: true, withSettings: true);
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    await tester.pumpAndSettle();
    expect(find.byKey(commandPaletteFieldKey), findsOneWidget);
    expect(find.text('Start recording'), findsOneWidget);

    await tester.enterText(find.byKey(commandPaletteFieldKey), 'proj');
    await tester.pumpAndSettle();
    expect(find.text('Start recording'), findsNothing);
    await tester.testTextInput.receiveAction(TextInputAction.done);
    await tester.pumpAndSettle();
    expect(find.text('Projects screen'), findsOneWidget);
  });

  testWidgets('? lists the shortcuts', (WidgetTester tester) async {
    await pumpHomeShell(tester, desktop);
    await typeKey(tester, LogicalKeyboardKey.slash, '?');
    expect(find.text('Keyboard shortcuts'), findsOneWidget);
    expect(find.text('Play or pause the recording'), findsOneWidget);
  });

  testWidgets(
      'single keys are text while typing in a field, Ctrl keys still work',
      (WidgetTester tester) async {
    int records = 0;
    int palettes = 0;
    await tester.pumpWidget(MaterialApp(
      home: Scaffold(
        body: AppShortcuts(
          bindings: <ShortcutActivator, VoidCallback>{
            const CharacterActivator('r'): () => records++,
            const SingleActivator(LogicalKeyboardKey.keyK, control: true): () =>
                palettes++,
          },
          child: const TextField(),
        ),
      ),
    ));

    await typeKey(tester, LogicalKeyboardKey.keyR, 'r');
    expect(records, 1, reason: 'nothing focused: R is a shortcut');

    await tester.tap(find.byType(TextField));
    await tester.pump();
    await typeKey(tester, LogicalKeyboardKey.keyR, 'r');
    expect(records, 1, reason: 'in the field R is text');
    await tester.sendKeyDownEvent(LogicalKeyboardKey.controlLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.keyK);
    await tester.sendKeyUpEvent(LogicalKeyboardKey.controlLeft);
    expect(palettes, 1);
  });
}
