// Hands a meeting to Claude or ChatGPT in the browser on a computer (#126), for owners who use
// claude.ai or chatgpt.com rather than Claude Code or Codex. The core writes a message that
// carries the prompt and the meeting (`inline`); the controller puts it on the clipboard. A
// message of up to `AiApp.maxPrefilledChars` goes in the website's address (`?q=`), which fills
// the message box (chatgpt.com also sends it); a longer one is pasted by the owner. Photos and files cannot be pasted, so their
// folder opens too, to drag them in. The answer comes back as on Android: the owner copies it,
// and Koalino reads the clipboard when its window is active again. Nothing is sent by Koalino.

import 'dart:io';

import 'package:flutter/services.dart';
import 'package:note_taker_ui/infrastructure/handoff_bridge.dart';
import 'package:path_provider/path_provider.dart';

/// Opens a web address or a folder with the system's default app.
typedef OpenWithSystem = Future<void> Function(String target);

/// Returns the app's data folder.
typedef DataDirectory = Future<String> Function();

/// The computer's side of a hand-off: the browser, the folder and the clipboard.
final class DesktopHandoffPlatform implements HandoffPlatform {
  /// Creates the platform side; `open` and `dataDirectory` are replaced in tests.
  const DesktopHandoffPlatform(
      {OpenWithSystem open = openWithSystem,
      DataDirectory dataDirectory = _supportDirectory})
      : _open = open,
        _dataDirectory = dataDirectory;

  final OpenWithSystem _open;
  final DataDirectory _dataDirectory;

  @override
  bool get viaBrowser => true;

  /// Under the app's support folder, emptied at start-up and by each hand-off.
  @override
  Future<String> handoffDirectory() async =>
      '${await _dataDirectory()}${Platform.pathSeparator}handoff';

  /// Both websites: every computer has a browser.
  @override
  Future<List<AiApp>> installedApps() async =>
      const <AiApp>[AiApp.claude, AiApp.chatgpt];

  /// Side effects: Opens the website in the browser, and the hand-off folder when it holds
  /// photos or files beside the meeting's Markdown.
  @override
  Future<void> share(PreparedHandoff handoff, AiApp? app) async {
    if (app == null) {
      return;
    }
    await _open(app.prefilledAddress(handoff.message) ?? app.webAddress);
    if (handoff.files.length > 1) {
      await _open(await handoffDirectory());
    }
  }

  /// A meeting short enough goes in the website's address, which fills its message box.
  @override
  bool prefills(PreparedHandoff handoff, AiApp? app) =>
      app?.prefilledAddress(handoff.message) != null;

  @override
  Future<String?> readClipboard() async =>
      (await Clipboard.getData(Clipboard.kTextPlain))?.text;

  @override
  Future<void> writeClipboard(String text) =>
      Clipboard.setData(ClipboardData(text: text));
}

/// The app's support folder.
Future<String> _supportDirectory() async =>
    (await getApplicationSupportDirectory()).path;

/// Opens `target`, a web address or a folder, with the system's default app.
///
/// Inputs: An `https://` address or an absolute folder path that Koalino chose itself.
/// Side effects: Starts `xdg-open` on Linux or Explorer's URL handler on Windows, detached.
/// Throws `PlatformException` when the system cannot open it.
Future<void> openWithSystem(String target) async {
  final (String executable, List<String> arguments) = Platform.isWindows
      ? ('rundll32', <String>['url.dll,FileProtocolHandler', target])
      : ('xdg-open', <String>[target]);
  try {
    await Process.start(executable, arguments, mode: ProcessStartMode.detached);
  } on ProcessException catch (error) {
    throw PlatformException(code: 'open_failed', message: error.message);
  }
}
