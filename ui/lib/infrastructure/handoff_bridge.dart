// Hands a meeting to the owner's own AI app (#114): Claude or ChatGPT, signed in with the owner's
// own account, which Koalino never holds. The core writes the meeting as Markdown plus its photos
// and files (`note_taker_handoff_meeting`) and keeps the answer as the summary (`keep_answer`,
// undone with `restore_summary`, through `note_taker_meeting_edit`). Android sends the message
// and files to the app (`note_taker/share` → `shareFiles`), and the clipboard carries the answer
// back. Meeting content and answers are never logged.

import 'dart:convert';
import 'dart:io';

import 'package:flutter/services.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:path_provider/path_provider.dart';

/// An AI app a meeting can be handed to directly.
enum AiApp {
  /// Anthropic's Claude app. Its website fills the message box from `?q=` and waits for the
  /// owner to send it (Claude Help Center, "Open Claude with a link").
  claude('com.anthropic.claude', 'Claude', 'https://claude.ai/new', 'claude.ai',
      false),

  /// OpenAI's ChatGPT app. Its website sends the message given in `?q=` by itself (seen on
  /// chatgpt.com in November 2026; not documented by OpenAI).
  chatgpt('com.openai.chatgpt', 'ChatGPT', 'https://chatgpt.com/',
      'chatgpt.com', true);

  const AiApp(this.packageName, this.displayName, this.webAddress, this.webName,
      this.sendsByItself);

  /// Longest message put in the website's address (#126); claude.ai cuts `?q=` at about 14,000
  /// characters, so a longer meeting is pasted instead.
  static const int maxPrefilledChars = 12000;

  /// Longest address opened, so the browser, the system's launcher (32,767 characters on
  /// Windows) and the website's servers all take it.
  static const int maxPrefilledAddressLength = 24000;

  /// Whether the website sends a message it was given in its address without the owner
  /// pressing Enter.
  final bool sendsByItself;

  /// The website's address with `message` in its message box, or null when the message is too
  /// long for an address.
  String? prefilledAddress(String message) {
    if (message.length > maxPrefilledChars) {
      return null;
    }
    final String address = '$webAddress?q=${Uri.encodeComponent(message)}';
    return address.length > maxPrefilledAddressLength ? null : address;
  }

  /// The Android package.
  final String packageName;

  /// The app's name, as shown.
  final String displayName;

  /// Where a computer opens it in the browser, for a new conversation (#126).
  final String webAddress;

  /// The website's name, as shown on a computer.
  final String webName;

  /// The app with `packageName`, or null.
  static AiApp? fromPackage(Object? packageName) {
    for (final AiApp app in values) {
      if (app.packageName == packageName) {
        return app;
      }
    }
    return null;
  }
}

/// What the other app is asked to do.
enum HandoffPurpose {
  /// Write the summary with the owner's prompt; the answer comes back.
  summary,

  /// Read the meeting and answer questions there.
  chat,
}

/// One file of a hand-off.
final class HandoffFile {
  /// Creates the file.
  const HandoffFile({required this.path, required this.mimeType});

  final String path;
  final String mimeType;
}

/// A hand-off the core wrote, ready to share.
final class PreparedHandoff {
  /// Creates the hand-off.
  const PreparedHandoff({
    required this.message,
    required this.prompt,
    required this.files,
    required this.skipped,
  });

  /// Reads the core's JSON.
  factory PreparedHandoff.fromJson(Map<String, Object?> json) =>
      PreparedHandoff(
        message: json['message'] as String? ?? '',
        prompt: json['prompt'] as String? ?? '',
        files: <HandoffFile>[
          for (final Object? file
              in json['files'] as List<Object?>? ?? <Object?>[])
            if (file case final Map<String, Object?> file)
              HandoffFile(
                path: file['path'] as String? ?? '',
                mimeType: file['mime_type'] as String? ?? '',
              ),
        ],
        skipped: (json['skipped'] as num?)?.toInt() ?? 0,
      );

  /// The text the share carries.
  final String message;

  /// For a summary, the filled-in instructions, kept with the answer; empty for a chat.
  final String prompt;

  /// The meeting's Markdown file first, then its photos and files.
  final List<HandoffFile> files;

  /// Files left out for size or count.
  final int skipped;
}

/// An answer kept as the summary, and the summary it replaced (as the core's JSON), for Undo.
final class KeptAnswer {
  /// Creates the result.
  const KeptAnswer({required this.summary, required this.previous});

  final StoredSummary summary;

  /// The summary before, as the core gave it, or null when there was none.
  final Map<String, Object?>? previous;
}

/// The core's side of a hand-off.
abstract interface class HandoffCore {
  /// Writes the hand-off into `directory`. Throws `LlmException`.
  Future<PreparedHandoff> prepare(
      String meetingId, HandoffPurpose purpose, String directory);

  /// Keeps an answer as the meeting's summary. Throws `LlmException`.
  Future<KeptAnswer> keepAnswer(String meetingId,
      {required String text, required String source, required String prompt});

  /// Puts back the summary an answer replaced; null removes it. Throws `LlmException`.
  Future<void> restore(String meetingId, Map<String, Object?>? previous);
}

/// The device's side of a hand-off: the folder, the apps, the share and the clipboard.
abstract interface class HandoffPlatform {
  /// Whether the apps are websites in the browser, where the owner pastes the message (#126).
  bool get viaBrowser;

  /// The folder the core writes hand-offs into.
  Future<String> handoffDirectory();

  /// Which AI apps are installed.
  Future<List<AiApp>> installedApps();

  /// Sends the message and files to `app`, or through the share sheet when null.
  Future<void> share(PreparedHandoff handoff, AiApp? app);

  /// Whether `share` puts the message in the app's message box itself, so the owner does not
  /// paste it (#126).
  bool prefills(PreparedHandoff handoff, AiApp? app);

  /// The clipboard's text, or null.
  Future<String?> readClipboard();

  /// Puts text on the clipboard.
  Future<void> writeClipboard(String text);
}

/// Calls the core.
final class RustHandoffCore implements HandoffCore {
  /// Creates the bridge; `call` is replaced in tests.
  ///
  /// `inline` asks for a message that carries the meeting, to paste into a website (#126).
  const RustHandoffCore({LlmNativeCall call = callRustLlm, bool inline = false})
      : _call = call,
        _inline = inline;

  final LlmNativeCall _call;
  final bool _inline;

  @override
  Future<PreparedHandoff> prepare(
          String meetingId, HandoffPurpose purpose, String directory) async =>
      PreparedHandoff.fromJson(
          decodeLlmAnswer(await _call('note_taker_handoff_meeting', <String?>[
        jsonEncode(<String, Object?>{
          'meeting_id': meetingId,
          'purpose': purpose.name,
          'directory': directory,
          if (_inline) 'inline': true,
        })
      ])));

  @override
  Future<KeptAnswer> keepAnswer(String meetingId,
      {required String text,
      required String source,
      required String prompt}) async {
    final Map<String, Object?> answer = await _edit(<String, Object?>{
      'command': 'keep_answer',
      'meeting_id': meetingId,
      'text': text,
      'source': source,
      'prompt': prompt,
    });
    return KeptAnswer(
      summary:
          StoredSummary.fromJson(answer['summary']! as Map<String, Object?>),
      previous: answer['previous'] as Map<String, Object?>?,
    );
  }

  @override
  Future<void> restore(String meetingId, Map<String, Object?>? previous) =>
      _edit(<String, Object?>{
        'command': 'restore_summary',
        'meeting_id': meetingId,
        'summary': previous,
      });

  Future<Map<String, Object?>> _edit(Map<String, Object?> request) async =>
      decodeLlmAnswer(await _call(
          'note_taker_meeting_edit', <String?>[jsonEncode(request)]));
}

/// Android's share sheet, the AI apps and the clipboard.
final class AndroidHandoffPlatform implements HandoffPlatform {
  /// Creates the platform side.
  const AndroidHandoffPlatform();

  static const MethodChannel _channel = MethodChannel('note_taker/share');

  @override
  bool get viaBrowser => false;

  /// The Android share carries the message to the app, which shows it ready to send.
  @override
  bool prefills(PreparedHandoff handoff, AiApp? app) => false;

  /// Under `cache/shared`, the folder Android's `SharedFileProvider` serves.
  @override
  Future<String> handoffDirectory() async =>
      '${(await getTemporaryDirectory()).path}/shared/handoff';

  @override
  Future<List<AiApp>> installedApps() async {
    final List<Object?> packages =
        await _channel.invokeListMethod<Object?>('installedApps') ??
            <Object?>[];
    return <AiApp>[
      for (final Object? package in packages)
        if (AiApp.fromPackage(package) case final AiApp app) app,
    ];
  }

  @override
  Future<void> share(PreparedHandoff handoff, AiApp? app) =>
      _channel.invokeMethod<void>('shareFiles', <String, Object?>{
        'paths': <String>[
          for (final HandoffFile file in handoff.files) file.path
        ],
        'text': handoff.message,
        if (app != null) 'packageName': app.packageName,
      });

  @override
  Future<String?> readClipboard() async =>
      (await Clipboard.getData(Clipboard.kTextPlain))?.text;

  @override
  Future<void> writeClipboard(String text) =>
      Clipboard.setData(ClipboardData(text: text));
}

/// Deletes what an earlier hand-off or shared export left in `directory`, as at start-up.
///
/// Side effects: Deletes plain copies of meeting content, files and folders; a failure is
/// ignored.
Future<void> clearHandoffFolder(String directory) async {
  try {
    final Directory folder = Directory(directory);
    if (await folder.exists()) {
      await for (final FileSystemEntity entry in folder.list()) {
        await entry.delete(recursive: true);
      }
    }
  } on FileSystemException {
    // Nothing to clear, or the next hand-off empties it.
  }
}
