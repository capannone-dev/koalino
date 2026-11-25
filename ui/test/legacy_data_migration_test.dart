// Specifies the move of desktop data from the preview's folders to the permanent app ID's (#168):
// the old folder's contents move once, a note is left behind, the newest of several old folders
// wins, and nothing moves when the new folder already holds data.

import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/legacy_data_migration.dart';

void main() {
  late Directory root;

  setUp(() => root = Directory.systemTemp.createTempSync('legacy-data-'));
  tearDown(() => root.deleteSync(recursive: true));

  File file(String relative) => File('${root.path}/$relative');

  void write(String relative, String contents) {
    file(relative)
      ..createSync(recursive: true)
      ..writeAsStringSync(contents);
  }

  Directory dir(String relative) => Directory('${root.path}/$relative');

  test('moves the old folder into the new one and leaves a note', () async {
    write('old/note_taker/note_taker.sqlite3', 'meetings');
    write('old/shared_preferences.json', '{}');

    final Directory? movedFrom = await migrateLegacyAppData(
      supportDirectory: dir('new'),
      legacySupportDirectories: <Directory>[dir('old')],
    );

    expect(movedFrom?.path, dir('old').path);
    expect(file('new/note_taker/note_taker.sqlite3').readAsStringSync(),
        'meetings');
    expect(file('new/shared_preferences.json').existsSync(), isTrue);
    expect(file('old/note_taker/note_taker.sqlite3').existsSync(), isFalse);
    expect(file('old/$legacyMovedNoteName').readAsStringSync(),
        contains(dir('new').path));
  });

  test('does nothing when the new folder already holds data', () async {
    write('old/note_taker/note_taker.sqlite3', 'old meetings');
    write('new/note_taker/note_taker.sqlite3', 'new meetings');

    final Directory? movedFrom = await migrateLegacyAppData(
      supportDirectory: dir('new'),
      legacySupportDirectories: <Directory>[dir('old')],
    );

    expect(movedFrom, isNull);
    expect(file('new/note_taker/note_taker.sqlite3').readAsStringSync(),
        'new meetings');
    expect(file('old/note_taker/note_taker.sqlite3').existsSync(), isTrue);
  });

  test('takes the most recently used of several old folders', () async {
    write('older/note_taker/note_taker.sqlite3', 'older');
    write('newer/note_taker/note_taker.sqlite3', 'newer');
    file('older/note_taker/note_taker.sqlite3')
        .setLastModifiedSync(DateTime(2026, 11, 1));
    file('newer/note_taker/note_taker.sqlite3')
        .setLastModifiedSync(DateTime(2026, 11, 20));

    final Directory? movedFrom = await migrateLegacyAppData(
      supportDirectory: dir('new'),
      legacySupportDirectories: <Directory>[dir('older'), dir('newer')],
    );

    expect(movedFrom?.path, dir('newer').path);
    expect(
        file('new/note_taker/note_taker.sqlite3').readAsStringSync(), 'newer');
    expect(file('older/note_taker/note_taker.sqlite3').existsSync(), isTrue);
  });

  test('does nothing on a new install', () async {
    expect(
      await migrateLegacyAppData(
        supportDirectory: dir('new'),
        legacySupportDirectories: <Directory>[dir('old')],
      ),
      isNull,
    );
  });

  test('names the preview folders of each desktop', () {
    expect(
      legacySupportDirectoriesFor(
              Directory('/home/u/.local/share/io.github.capannone_dev.koalino'),
              'linux')
          .map((Directory d) => d.path),
      <String>['/home/u/.local/share/com.example.note_taker_ui'],
    );
    expect(
      legacySupportDirectoriesFor(
              Directory(
                  '/Users/u/Library/Application Support/io.github.capannone-dev.koalino'),
              'macos')
          .map((Directory d) => d.path),
      <String>['/Users/u/Library/Application Support/com.example.noteTakerUi'],
    );
    expect(
      legacySupportDirectoriesFor(
              Directory(r'C:\Users\u\AppData\Roaming\capannone-dev\Koalino'),
              'windows')
          .map((Directory d) => d.path),
      <String>[
        r'C:\Users\u\AppData\Roaming\com.example\Koalino',
        r'C:\Users\u\AppData\Roaming\com.example\note_taker_ui',
      ],
    );
    expect(
        legacySupportDirectoriesFor(Directory('/data/x'), 'android'), isEmpty);
  });
}
