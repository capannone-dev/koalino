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
  /// Anthropic's Claude app.
  claude('com.anthropic.claude', 'Claude'),

  /// OpenAI's ChatGPT app.
  chatgpt('com.openai.chatgpt', 'ChatGPT');

  const AiApp(this.packageName, this.displayName);

  /// The Android package.
  final String packageName;

  /// The app's name, as shown.
  final String displayName;

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

/// The phone's side of a hand-off: the folder, the apps, the share and the clipboard.
abstract interface class HandoffPlatform {
  /// The folder the core writes hand-offs into.
  Future<String> handoffDirectory();

  /// Which AI apps are installed.
  Future<List<AiApp>> installedApps();

  /// Sends the message and files to `app`, or through the share sheet when null.
  Future<void> share(PreparedHandoff handoff, AiApp? app);

  /// The clipboard's text, or null.
  Future<String?> readClipboard();

  /// Puts text on the clipboard.
  Future<void> writeClipboard(String text);
}

/// Calls the core.
final class RustHandoffCore implements HandoffCore {
  /// Creates the bridge; `call` is replaced in tests.
  const RustHandoffCore({LlmNativeCall call = callRustLlm}) : _call = call;

  final LlmNativeCall _call;

  @override
  Future<PreparedHandoff> prepare(
          String meetingId, HandoffPurpose purpose, String directory) async =>
      PreparedHandoff.fromJson(
          decodeLlmAnswer(await _call('note_taker_handoff_meeting', <String?>[
        jsonEncode(<String, Object?>{
          'meeting_id': meetingId,
          'purpose': purpose.name,
          'directory': directory,
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

/// Deletes the files an earlier hand-off left in `directory`, as at start-up.
///
/// Side effects: Deletes plain copies of meeting content; a failure is ignored.
Future<void> clearHandoffFolder(String directory) async {
  try {
    final Directory folder = Directory(directory);
    if (await folder.exists()) {
      await for (final FileSystemEntity entry in folder.list()) {
        if (entry is File) {
          await entry.delete();
        }
      }
    }
  } on FileSystemException {
    // Nothing to clear, or the next hand-off empties it.
  }
}
