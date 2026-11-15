// Specifies the computer's hand-off (#126): with no Claude Code or Codex, the owner uses
// claude.ai or chatgpt.com in the browser. The meeting goes on the clipboard inside the message,
// the website opens, and the folder opens only when there are photos or files to drag in.

import 'package:flutter_test/flutter_test.dart';
import 'package:note_taker_ui/infrastructure/desktop_handoff.dart';
import 'package:note_taker_ui/infrastructure/handoff_bridge.dart';

/// A hand-off with `extraFiles` photos or files beside the Markdown.
PreparedHandoff _handoff(
        {int extraFiles = 0, String message = 'Summarize it.\n\n# Review'}) =>
    PreparedHandoff(
      message: message,
      prompt: 'Summarize it.',
      files: <HandoffFile>[
        const HandoffFile(
            path: '/data/handoff/Review.md', mimeType: 'text/markdown'),
        for (int index = 0; index < extraFiles; index++)
          HandoffFile(
              path: '/data/handoff/photo$index.jpg', mimeType: 'image/jpeg'),
      ],
      skipped: 0,
    );

void main() {
  late List<String> opened;
  late DesktopHandoffPlatform platform;

  setUp(() {
    opened = <String>[];
    platform = DesktopHandoffPlatform(
      open: (String target) async => opened.add(target),
      dataDirectory: () async => '/data',
    );
  });

  test('both websites are offered, through the browser', () async {
    expect(
        await platform.installedApps(), <AiApp>[AiApp.claude, AiApp.chatgpt]);
    expect(platform.viaBrowser, isTrue);
    expect(await platform.handoffDirectory(), '/data/handoff');
  });

  test('a short meeting opens the website with it in the message box',
      () async {
    await platform.share(_handoff(), AiApp.claude);
    await platform.share(_handoff(), AiApp.chatgpt);

    expect(opened, <String>[
      'https://claude.ai/new?q=Summarize%20it.%0A%0A%23%20Review',
      'https://chatgpt.com/?q=Summarize%20it.%0A%0A%23%20Review',
    ]);
    expect(platform.prefills(_handoff(), AiApp.claude), isTrue);
  });

  test('a long meeting opens the website alone, to paste it', () async {
    final PreparedHandoff long =
        _handoff(message: 'x' * (AiApp.maxPrefilledChars + 1));
    // Accented letters take six characters each in the address.
    final PreparedHandoff accented =
        _handoff(message: 'è' * (AiApp.maxPrefilledChars ~/ 2));

    await platform.share(long, AiApp.claude);
    await platform.share(accented, AiApp.chatgpt);

    expect(opened, <String>['https://claude.ai/new', 'https://chatgpt.com/']);
    expect(platform.prefills(long, AiApp.claude), isFalse);
    expect(platform.prefills(accented, AiApp.chatgpt), isFalse);
  });

  test('photos and files open their folder to drag them in', () async {
    await platform.share(_handoff(extraFiles: 2), AiApp.claude);

    expect(opened.last, '/data/handoff');
  });

  test(
      'chatgpt.com, which sends at once, is not pre-filled when files must go in first',
      () async {
    await platform.share(_handoff(extraFiles: 1), AiApp.chatgpt);

    expect(opened.first, 'https://chatgpt.com/');
    expect(platform.prefills(_handoff(extraFiles: 1), AiApp.chatgpt), isFalse);
    expect(platform.prefills(_handoff(extraFiles: 1), AiApp.claude), isTrue);
  });

  test('the core is asked for a message that carries the meeting', () async {
    final List<String?> sent = <String?>[];
    final RustHandoffCore core = RustHandoffCore(
      inline: true,
      call: (String function, List<String?> arguments) async {
        sent.addAll(arguments);
        return '{"ok":true,"message":"m","prompt":"p","files":[],"skipped":0}';
      },
    );

    await core.prepare('meeting-1', HandoffPurpose.summary, '/data/handoff');

    expect(sent.single, contains('"inline":true'));
  });
}
