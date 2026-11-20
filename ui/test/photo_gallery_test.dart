// Specifies the full-screen viewer of a meeting's photos and screenshots (#144): the arrow keys
// and a sideways swipe move between them, Esc and a swipe down close it, and a zoomed picture
// pans instead of turning the page.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/presentation/photo_gallery.dart';

const List<String> _names = <String>['first.png', 'second.png', 'third.png'];

/// Opens the gallery on `initialIndex` from a home screen, as the meeting screen does.
Future<void> _open(WidgetTester tester, {int initialIndex = 0}) async {
  await tester.pumpWidget(MaterialApp(
    home: Builder(
      builder: (BuildContext context) => Scaffold(
        body: Center(
          child: TextButton(
            onPressed: () => Navigator.of(context).push(MaterialPageRoute<void>(
              builder: (BuildContext context) => PhotoGallery(
                count: _names.length,
                initialIndex: initialIndex,
                titleOf: (int index) => _names[index],
                photoBuilder: (BuildContext context, int index) => SizedBox(
                    key: ValueKey<String>('photo-$index'),
                    width: 300,
                    height: 300,
                    child: const ColoredBox(color: Colors.white)),
              ),
            )),
            child: const Text('home'),
          ),
        ),
      ),
    ),
  ));
  await tester.tap(find.text('home'));
  await tester.pumpAndSettle();
}

/// The title shown: the photo's name and its place among the meeting's photos.
String? _title(WidgetTester tester) => tester
    .widgetList<Text>(
        find.descendant(of: find.byType(AppBar), matching: find.byType(Text)))
    .map((Text text) => text.data)
    .join(' ');

void main() {
  testWidgets('the arrow keys go to the next and previous photo',
      (WidgetTester tester) async {
    await _open(tester, initialIndex: 1);
    expect(_title(tester), contains('second.png'));
    expect(_title(tester), contains('2 / 3'));

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pumpAndSettle();
    expect(_title(tester), contains('third.png'));

    // The last photo stays put.
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowRight);
    await tester.pumpAndSettle();
    expect(_title(tester), contains('third.png'));

    await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
    await tester.sendKeyEvent(LogicalKeyboardKey.arrowLeft);
    await tester.pumpAndSettle();
    expect(_title(tester), contains('first.png'));
  });

  testWidgets('Esc closes the viewer', (WidgetTester tester) async {
    await _open(tester);
    await tester.sendKeyEvent(LogicalKeyboardKey.escape);
    await tester.pumpAndSettle();
    expect(find.byType(PhotoGallery), findsNothing);
    expect(find.text('home'), findsOneWidget);
  });

  testWidgets('a sideways swipe turns the page', (WidgetTester tester) async {
    await _open(tester);
    await tester.fling(find.byKey(const ValueKey<String>('photo-0')),
        const Offset(-400, 0), 1500);
    await tester.pumpAndSettle();
    expect(_title(tester), contains('second.png'));

    await tester.fling(find.byKey(const ValueKey<String>('photo-1')),
        const Offset(400, 0), 1500);
    await tester.pumpAndSettle();
    expect(_title(tester), contains('first.png'));
  });

  testWidgets('a swipe down closes the viewer', (WidgetTester tester) async {
    await _open(tester);
    await tester.fling(find.byKey(const ValueKey<String>('photo-0')),
        const Offset(0, 400), 1500);
    await tester.pumpAndSettle();
    expect(find.byType(PhotoGallery), findsNothing);
  });

  testWidgets('a short drag down does not close it',
      (WidgetTester tester) async {
    await _open(tester);
    await tester.timedDrag(find.byKey(const ValueKey<String>('photo-0')),
        const Offset(0, 40), const Duration(milliseconds: 600));
    await tester.pumpAndSettle();
    expect(find.byType(PhotoGallery), findsOneWidget);
  });

  testWidgets('the buttons go to the next and previous photo',
      (WidgetTester tester) async {
    await _open(tester);
    expect(find.byTooltip('Previous page'), findsNothing);
    await tester.tap(find.byTooltip('Next page'));
    await tester.pumpAndSettle();
    expect(_title(tester), contains('second.png'));
    await tester.tap(find.byTooltip('Previous page'));
    await tester.pumpAndSettle();
    expect(_title(tester), contains('first.png'));
  });

  testWidgets('one photo has no page buttons or count',
      (WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      home: PhotoGallery(
        count: 1,
        initialIndex: 0,
        titleOf: (int index) => 'only.png',
        photoBuilder: (BuildContext context, int index) =>
            const SizedBox(width: 100, height: 100),
      ),
    ));
    expect(find.byTooltip('Next page'), findsNothing);
    expect(_title(tester), 'only.png');
  });

  testWidgets('a pinch zooms the photo instead of turning the page',
      (WidgetTester tester) async {
    await _open(tester);
    final Finder photo = find.byKey(const ValueKey<String>('photo-0'));
    final double before = tester.getRect(photo).width;
    final Offset center = tester.getCenter(photo);

    // Two fingers land; the thumb stays nearly still while the other finger moves away, as in a
    // real pinch (fingers moving exactly opposite cancel out and hide the problem).
    final TestGesture left =
        await tester.startGesture(center - const Offset(20, 0), pointer: 1);
    final TestGesture right =
        await tester.startGesture(center + const Offset(20, 0), pointer: 2);
    for (int step = 0; step < 10; step++) {
      await left.moveBy(const Offset(-1, 1));
      await right.moveBy(const Offset(14, 6));
      await tester.pump(const Duration(milliseconds: 16));
    }
    await left.up();
    await right.up();
    await tester.pumpAndSettle();

    expect(_title(tester), contains('first.png'));
    expect(tester.getRect(photo).width, greaterThan(before * 1.5));
  });
}
