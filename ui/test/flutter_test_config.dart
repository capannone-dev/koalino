// Runs before every test file: viewers get clear copies (#99) from a fake that answers at once
// with a path under `/clear-copies/`, since tests load no native core. A test can replace
// `clearCopyResolver` for its own cases.

import 'dart:async';

import 'package:note_taker_ui/infrastructure/clear_copy_bridge.dart';

/// Sets the fake resolver, then runs the test file.
Future<void> testExecutable(FutureOr<void> Function() testMain) async {
  clearCopyResolver =
      (String attachmentId) async => '/clear-copies/$attachmentId';
  await testMain();
}
