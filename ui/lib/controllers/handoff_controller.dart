// Hands meetings to the owner's own AI app and brings a summary back (#114).
//
// A summary hand-off stays pending for an hour. Android gives no app another app's reply, so
// when Koalino comes back to the foreground it reads the clipboard: new text of a summary's length
// is kept as that meeting's summary, and an event offers Undo. The prompt is put on the clipboard
// when sharing, so an app that drops the shared text can still be given it, and so the clipboard
// still holding it means nothing was copied. An answer shared back to Koalino as text is offered
// as the summary too (`keepSharedAnswer`). A chat hand-off leaves nothing pending. On a computer
// (#126) the apps are claude.ai and chatgpt.com in the browser: the steps are shown first, the
// message carries the meeting (in the website's address when short enough, else pasted by the
// owner), and the clipboard is read when Koalino's window is active again.

import 'dart:async';

import 'package:flutter/services.dart';
import 'package:flutter/widgets.dart';
import 'package:note_taker_ui/infrastructure/handoff_bridge.dart';
import 'package:note_taker_ui/infrastructure/llm_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Least length of copied text taken for a summary, so a stray word is not kept.
const int minAnswerCharacters = 80;

/// How long a summary hand-off waits for its answer.
const Duration answerWindow = Duration(hours: 1);

/// What the owner will do in the website, shown before it opens (#126).
final class HandoffSteps {
  /// Creates the steps.
  const HandoffSteps({
    required this.app,
    required this.purpose,
    required this.prefilled,
    required this.hasFiles,
  });

  /// The website; null for the share sheet.
  final AiApp? app;

  /// A summary (the answer comes back) or a chat.
  final HandoffPurpose purpose;

  /// Whether the website opens with the meeting in its message box; otherwise the owner pastes.
  final bool prefilled;

  /// Whether a folder of photos and files opens too, to drag them in.
  final bool hasFiles;
}

/// A summary hand-off waiting for its answer.
final class PendingAnswer {
  /// Creates the pending hand-off.
  const PendingAnswer({
    required this.meetingId,
    required this.title,
    required this.app,
    required this.prompt,
    required this.sharedText,
    required this.startedAt,
  });

  final String meetingId;
  final String title;

  /// The app it went to, or null for one picked in the share sheet.
  final AiApp? app;

  /// The filled-in instructions, kept with the answer.
  final String prompt;

  /// What the clipboard held after sharing; the same text means nothing was copied.
  final String sharedText;
  final DateTime startedAt;
}

/// Something to tell the owner after coming back.
sealed class HandoffEvent {
  const HandoffEvent();
}

/// An answer was kept as a meeting's summary; `undo` puts the earlier one back.
final class HandoffKept extends HandoffEvent {
  /// Creates the event.
  const HandoffKept(
      {required this.meetingId,
      required this.title,
      required this.appName,
      required this.previous});

  final String meetingId;
  final String title;

  /// Such as `Claude`, or `the other app`.
  final String appName;

  /// The summary it replaced, as the core gave it, or null.
  final Map<String, Object?>? previous;
}

/// Came back without copying the answer: how to keep it.
final class HandoffHint extends HandoffEvent {
  /// Creates the event.
  const HandoffHint({required this.appName});

  final String appName;
}

/// Keeping or undoing an answer failed.
final class HandoffFailed extends HandoffEvent {
  /// Creates the event.
  const HandoffFailed(this.message);

  final String message;
}

/// Hands meetings over and keeps answers.
final class HandoffController extends ChangeNotifier
    with WidgetsBindingObserver {
  /// Creates the controller.
  ///
  /// Inputs: The core and phone ports; `now` for the clock (tests); `onSummaryChanged`, told the
  /// meeting whose summary changed so its views reload it.
  HandoffController({
    required HandoffCore core,
    required HandoffPlatform platform,
    DateTime Function()? now,
    void Function(String meetingId)? onSummaryChanged,
  })  : _core = core,
        _platform = platform,
        _now = now ?? DateTime.now,
        _onSummaryChanged = onSummaryChanged;

  final HandoffCore _core;
  final HandoffPlatform _platform;
  final DateTime Function() _now;
  final void Function(String meetingId)? _onSummaryChanged;
  final StreamController<HandoffEvent> _events =
      StreamController<HandoffEvent>.broadcast();
  bool _hinted = false;
  bool _checking = false;

  /// Whether the apps are websites in the browser, where the owner pastes the meeting (#126).
  bool get viaBrowser => _platform.viaBrowser;

  /// The AI apps installed, for direct hand-offs; others go through the share sheet.
  List<AiApp> apps = <AiApp>[];

  /// The summary hand-off waiting for its answer, if any.
  PendingAnswer? pending;

  /// What to tell the owner.
  Stream<HandoffEvent> get events => _events.stream;

  /// Starts watching for the app coming back to the foreground.
  void attach() => WidgetsBinding.instance.addObserver(this);

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    if (state == AppLifecycleState.resumed) {
      unawaited(checkClipboard());
    }
  }

  /// Finds which AI apps are installed. Side effects: Asks the platform; notifies listeners.
  Future<void> loadApps() async {
    try {
      apps = await _platform.installedApps();
    } on PlatformException {
      apps = <AiApp>[];
    }
    notifyListeners();
  }

  /// Hands a meeting to `app`, or to one picked in the share sheet.
  ///
  /// Inputs: The meeting, the purpose, the app, and `confirm`, which is shown the steps once the
  /// meeting is ready and may cancel (on a computer, #126).
  /// Outputs: null once shared or cancelled, or a message why not. Side effects: The core writes
  /// the meeting and its files to the hand-off folder; the message goes on the clipboard; the
  /// other app opens with them. A summary hand-off becomes `pending`, replacing an earlier one.
  Future<String?> handOff(
      String meetingId, String title, HandoffPurpose purpose,
      {AiApp? app, Future<bool> Function(HandoffSteps steps)? confirm}) async {
    try {
      final PreparedHandoff handoff = await _core.prepare(
          meetingId, purpose, await _platform.handoffDirectory());
      if (confirm != null &&
          !await confirm(HandoffSteps(
            app: app,
            purpose: purpose,
            prefilled: _platform.prefills(handoff, app),
            hasFiles: handoff.hasFilesToDrag,
          ))) {
        return null;
      }
      await _platform.writeClipboard(handoff.message);
      await _platform.share(handoff, app);
      pending = purpose == HandoffPurpose.summary
          ? PendingAnswer(
              meetingId: meetingId,
              title: title,
              app: app,
              prompt: handoff.prompt,
              sharedText: handoff.message,
              startedAt: _now(),
            )
          : null;
      _hinted = false;
      notifyListeners();
      return null;
    } on LlmException catch (error) {
      return error.message;
    } on PlatformException {
      return l10n.theOtherAppCouldNotBe;
    }
  }

  /// Hands a project's chat, or every note's when `projectId` is null, to `app`, or to one
  /// picked in the share sheet (#126).
  ///
  /// Inputs: The project or none, the app, and `confirm` as in `handOff`. Outputs: null once
  /// shared or cancelled, or a message why not. Side effects: The core writes the meetings and
  /// every file of the projects and meetings to the hand-off folder; the message goes on the
  /// clipboard; the other app opens with them. Nothing comes back, so no answer is awaited.
  Future<String?> handOffNotes(
      {String? projectId,
      AiApp? app,
      Future<bool> Function(HandoffSteps steps)? confirm}) async {
    try {
      final PreparedHandoff handoff = await _core.prepareNotes(
          projectId, await _platform.handoffDirectory());
      if (confirm != null &&
          !await confirm(HandoffSteps(
            app: app,
            purpose: HandoffPurpose.chat,
            prefilled: _platform.prefills(handoff, app),
            hasFiles: handoff.hasFilesToDrag,
          ))) {
        return null;
      }
      await _platform.writeClipboard(handoff.message);
      await _platform.share(handoff, app);
      pending = null;
      _hinted = false;
      notifyListeners();
      return null;
    } on LlmException catch (error) {
      return error.message;
    } on PlatformException {
      return l10n.theOtherAppCouldNotBe;
    }
  }

  /// Keeps a copied answer as the pending meeting's summary, if there is one.
  ///
  /// Side effects: Reads the clipboard; may write the meeting's summary (which syncs) and add an
  /// event. A hand-off older than `answerWindow` is dropped.
  Future<void> checkClipboard() async {
    final PendingAnswer? waiting = pending;
    if (waiting == null || _checking) {
      return;
    }
    if (_now().difference(waiting.startedAt) > answerWindow) {
      pending = null;
      notifyListeners();
      return;
    }
    _checking = true;
    try {
      final String text = (await _platform.readClipboard())?.trim() ?? '';
      final String appName = waiting.app?.displayName ?? l10n.theOtherApp;
      if (text == waiting.sharedText.trim() || text.isEmpty) {
        if (!_hinted) {
          _hinted = true;
          _events.add(HandoffHint(appName: appName));
        }
        return;
      }
      if (text.length < minAnswerCharacters) {
        return;
      }
      await _keep(waiting, text);
    } on PlatformException {
      // The clipboard could not be read; the hand-off stays pending.
    } finally {
      _checking = false;
    }
  }

  /// Keeps text shared back to Koalino as the pending meeting's summary.
  ///
  /// Outputs: true when it was taken (kept, or failed with a `HandoffFailed` event); false when
  /// no summary waits or the text is shorter than `minAnswerCharacters`.
  /// Side effects: As `checkClipboard`, without reading the clipboard.
  Future<bool> keepSharedAnswer(String sharedText) async {
    final PendingAnswer? waiting = pending;
    final String text = sharedText.trim();
    if (waiting == null ||
        _now().difference(waiting.startedAt) > answerWindow ||
        text.length < minAnswerCharacters) {
      return false;
    }
    await _keep(waiting, text);
    return true;
  }

  /// Keeps `text` as `waiting`'s summary and adds the event that tells the owner.
  Future<void> _keep(PendingAnswer waiting, String text) async {
    pending = null;
    notifyListeners();
    try {
      final KeptAnswer kept = await _core.keepAnswer(waiting.meetingId,
          text: text,
          // On a computer the answer came from the website (#126).
          source: switch (waiting.app) {
            final AiApp app when viaBrowser => app.webName,
            final AiApp app => l10n.app(app.displayName),
            null => l10n.app('Another'),
          },
          prompt: waiting.prompt);
      _onSummaryChanged?.call(waiting.meetingId);
      _events.add(HandoffKept(
          meetingId: waiting.meetingId,
          title: waiting.title,
          appName: waiting.app?.displayName ?? l10n.theOtherApp,
          previous: kept.previous));
    } on LlmException catch (error) {
      _events.add(HandoffFailed(error.message));
    }
  }

  /// Puts back the summary that `kept` replaced.
  ///
  /// Outputs: null once undone, or why not. Side effects: Writes the meeting's summary.
  Future<String?> undo(HandoffKept kept) async {
    try {
      await _core.restore(kept.meetingId, kept.previous);
      _onSummaryChanged?.call(kept.meetingId);
      return null;
    } on LlmException catch (error) {
      return error.message;
    }
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    unawaited(_events.close());
    super.dispose();
  }
}
