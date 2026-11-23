// Specifies where the app looks for the Rust core on macOS (#151): inside its own bundle.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/core_library.dart';

void main() {
  test('on macOS the core is in the app bundle\'s Frameworks folder', () {
    expect(
        macosCoreLibraryPath(
            '/Applications/Koalino.app/Contents/MacOS/Koalino'),
        '/Applications/Koalino.app/Contents/Frameworks/libnote_taker_core.dylib');
  });

  test('a bundle outside Applications works the same way', () {
    expect(
        macosCoreLibraryPath(
            '/Users/me/Downloads/Koalino.app/Contents/MacOS/note_taker_ui'),
        '/Users/me/Downloads/Koalino.app/Contents/Frameworks/'
        'libnote_taker_core.dylib');
  });
}
