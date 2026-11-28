// Specifies the file that remembers dismissed tips (#213).

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/seen_tips_store.dart';

void main() {
  test('a dismissed tip is remembered across stores, others are not', () async {
    final Directory folder = await Directory.systemTemp.createTemp('tips');
    addTearDown(() => folder.delete(recursive: true));
    final FileSeenTipsStore store =
        FileSeenTipsStore(directory: () async => folder);

    expect(await store.seen('a'), isFalse);
    await store.markSeen('a');
    await store.markSeen('b');

    final FileSeenTipsStore again =
        FileSeenTipsStore(directory: () async => folder);
    expect(await again.seen('a'), isTrue);
    expect(await again.seen('b'), isTrue);
    expect(await again.seen('c'), isFalse);
  });

  test('an unreadable folder means not seen, and saving does not throw',
      () async {
    final FileSeenTipsStore store = FileSeenTipsStore(
        directory: () async => throw const FileSystemException('none'));

    expect(await store.seen('a'), isFalse);
    await store.markSeen('a');
  });
}
