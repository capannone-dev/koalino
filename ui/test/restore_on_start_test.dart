// Specifies the start-up swap of a staged restore (#167): a ready staging folder replaces the
// data folder, the old data is kept aside with the downloaded models moved back in, and an
// unfinished staging folder is discarded without touching the data.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/restore_on_start.dart';

void main() {
  late Directory support;

  setUp(
      () => support = Directory.systemTemp.createTempSync('restore-on-start-'));
  tearDown(() => support.deleteSync(recursive: true));

  File file(String relative) => File('${support.path}/$relative');

  void write(String relative, String contents) {
    file(relative)
      ..createSync(recursive: true)
      ..writeAsStringSync(contents);
  }

  test('does nothing without a staged restore', () async {
    write('note_taker/note_taker.sqlite3', 'current');

    expect(await applyPendingRestore(support), PendingRestore.none);
    expect(file('note_taker/note_taker.sqlite3').readAsStringSync(), 'current');
  });

  test('swaps a ready restore in and keeps the old data aside', () async {
    write('note_taker/note_taker.sqlite3', 'current');
    write('note_taker/models/ggml-small.bin', 'downloaded model');
    write('note_taker.restore/note_taker.sqlite3', 'restored');
    write('note_taker.restore/keys/encryption.key.wrapped', 'wrapped');
    write('note_taker.restore/$restoreReadyMarker', 'ready');

    expect(await applyPendingRestore(support), PendingRestore.applied);

    expect(
        file('note_taker/note_taker.sqlite3').readAsStringSync(), 'restored');
    expect(file('note_taker/keys/encryption.key.wrapped').existsSync(), isTrue);
    expect(file('note_taker/$restoreReadyMarker').existsSync(), isFalse);
    // Models are not in a backup, so the ones already on the device move in.
    expect(file('note_taker/models/ggml-small.bin').readAsStringSync(),
        'downloaded model');
    expect(
        file('note_taker.before-restore/note_taker.sqlite3').readAsStringSync(),
        'current');
    expect(
        Directory('${support.path}/note_taker.restore').existsSync(), isFalse);
  });

  test('replaces the copy an earlier restore kept aside', () async {
    write('note_taker.before-restore/old.txt', 'older');
    write('note_taker/note_taker.sqlite3', 'current');
    write('note_taker.restore/note_taker.sqlite3', 'restored');
    write('note_taker.restore/$restoreReadyMarker', 'ready');

    expect(await applyPendingRestore(support), PendingRestore.applied);

    expect(file('note_taker.before-restore/old.txt').existsSync(), isFalse);
    expect(
        file('note_taker.before-restore/note_taker.sqlite3').readAsStringSync(),
        'current');
  });

  test('applies a restore on a device without data', () async {
    write('note_taker.restore/note_taker.sqlite3', 'restored');
    write('note_taker.restore/$restoreReadyMarker', 'ready');

    expect(await applyPendingRestore(support), PendingRestore.applied);

    expect(
        file('note_taker/note_taker.sqlite3').readAsStringSync(), 'restored');
  });

  test('discards an unfinished restore and keeps the data', () async {
    write('note_taker/note_taker.sqlite3', 'current');
    write('note_taker.restore/note_taker.sqlite3', 'half written');

    expect(await applyPendingRestore(support), PendingRestore.discarded);

    expect(file('note_taker/note_taker.sqlite3').readAsStringSync(), 'current');
    expect(
        Directory('${support.path}/note_taker.restore').existsSync(), isFalse);
  });
}
