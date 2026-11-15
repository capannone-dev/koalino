// Hands a meeting to the owner's own AI app (#114) on Android. The sheet offers Summarize and Chat
// in the Claude or ChatGPT app when installed, or in any app through the share sheet. On a
// computer (#126) it offers claude.ai and chatgpt.com in the browser instead, and numbered steps
// say what to do there before the website opens. The listener
// around the app's screens tells the owner when an answer was kept as a summary, with Undo, or
// how to keep it.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/handoff_controller.dart';
import 'package:note_taker_ui/infrastructure/chat_bridge.dart';
import 'package:note_taker_ui/infrastructure/handoff_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// One choice of the hand-off sheet.
final class _HandoffChoice {
  const _HandoffChoice(this.purpose, this.app);
  final HandoffPurpose purpose;

  /// Null for the share sheet.
  final AiApp? app;
}

/// Asks where to hand the meeting and hands it over.
///
/// Inputs: A context with a `ScaffoldMessenger`, the controller and the meeting. Side effects:
/// Writes the meeting's hand-off and opens the other app; a failure is shown in a snack bar.
Future<void> showHandoffSheet(
  BuildContext context, {
  required HandoffController controller,
  required String meetingId,
  required String title,
}) async {
  final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
  final _HandoffChoice? choice =
      await _pickHandoff(context, controller, purposes: HandoffPurpose.values);
  if (choice == null || !context.mounted) {
    return;
  }
  final String? problem = await controller.handOff(
      meetingId, title, choice.purpose,
      app: choice.app, confirm: _stepsFor(context, controller));
  if (problem != null) {
    messenger.showSnackBar(SnackBar(content: Text(problem)));
  }
}

/// Asks where to continue a chat, and hands it over (#126).
///
/// Inputs: A context with a `ScaffoldMessenger`, the controller, and the chat's topic: a
/// meeting, a project, or every note. Side effects: Writes the hand-off (a project's or every
/// note's goes with all their files) and opens the other app; a failure is shown in a snack bar.
Future<void> showChatHandoffSheet(
  BuildContext context, {
  required HandoffController controller,
  required ChatTopic topic,
}) async {
  final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
  final _HandoffChoice? choice = await _pickHandoff(context, controller,
      purposes: const <HandoffPurpose>[HandoffPurpose.chat],
      intro: l10n.chatHandoffIntro);
  if (choice == null || !context.mounted) {
    return;
  }
  final Future<bool> Function(HandoffSteps steps)? confirm =
      _stepsFor(context, controller);
  final String? problem = switch (topic) {
    ChatTopic(scope: 'meeting', :final String id) => await controller.handOff(
        id, '', HandoffPurpose.chat,
        app: choice.app, confirm: confirm),
    ChatTopic(scope: 'project', :final String id) => await controller
        .handOffNotes(projectId: id, app: choice.app, confirm: confirm),
    _ => await controller.handOffNotes(app: choice.app, confirm: confirm),
  };
  if (problem != null) {
    messenger.showSnackBar(SnackBar(content: Text(problem)));
  }
}

/// On a computer, the steps window shown before the website opens; null on a phone.
Future<bool> Function(HandoffSteps steps)? _stepsFor(
        BuildContext context, HandoffController controller) =>
    controller.viaBrowser
        ? (HandoffSteps steps) async =>
            context.mounted && await showHandoffSteps(context, steps)
        : null;

/// Shows `intro` (or the meeting's), the apps for `purposes`, and the share sheet on a phone.
///
/// Outputs: The owner's choice, or null when dismissed. Side effects: None.
Future<_HandoffChoice?> _pickHandoff(
  BuildContext context,
  HandoffController controller, {
  required List<HandoffPurpose> purposes,
  String? intro,
}) =>
    showModalBottomSheet<_HandoffChoice>(
      context: context,
      showDragHandle: true,
      builder: (BuildContext context) => SafeArea(
        child: ListView(
          shrinkWrap: true,
          children: <Widget>[
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 0, 24, 8),
              child: Text(l10n.useYourOwnAiApp,
                  style: Theme.of(context).textTheme.titleLarge),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(24, 0, 24, 8),
              child: Text(intro ??
                  (controller.viaBrowser
                      ? l10n.handoffInTheBrowser
                      : l10n.theMeetingItsNotesAndIts)),
            ),
            for (final AiApp app in controller.apps)
              for (final HandoffPurpose purpose in purposes)
                ListTile(
                  leading: Icon(purpose == HandoffPurpose.summary
                      ? Icons.summarize_outlined
                      : Icons.forum_outlined),
                  title: Text(_appAction(purpose,
                      controller.viaBrowser ? app.webName : app.displayName)),
                  onTap: () =>
                      Navigator.of(context).pop(_HandoffChoice(purpose, app)),
                ),
            // A computer has no share sheet to reach another app.
            if (!controller.viaBrowser)
              for (final HandoffPurpose purpose in purposes)
                ListTile(
                  leading: const Icon(Icons.share_outlined),
                  title: Text(purpose == HandoffPurpose.summary
                      ? l10n.summarizeInAnotherApp
                      : l10n.chatInAnotherApp),
                  onTap: () =>
                      Navigator.of(context).pop(_HandoffChoice(purpose, null)),
                ),
          ],
        ),
      ),
    );

/// Such as `Summarize in Claude` or `Chat in chatgpt.com`.
String _appAction(HandoffPurpose purpose, String name) =>
    purpose == HandoffPurpose.summary
        ? l10n.summarizeIn(name)
        : l10n.chatIn(name);

/// Shows what the owner will do in the website, before it opens (#126).
///
/// Inputs: A context below a `Navigator`, and the steps. Outputs: true to open the website,
/// false on Cancel or Escape. Side effects: None.
Future<bool> showHandoffSteps(BuildContext context, HandoffSteps steps) async {
  final String site = steps.app?.webName ?? l10n.theOtherApp;
  final bool summary = steps.purpose == HandoffPurpose.summary;
  // The photos and files go into the message box before the message is sent.
  final List<String> lines = <String>[
    if (!steps.hasFiles) ...<String>[
      if (!steps.prefilled)
        l10n.handoffStepPaste(site)
      else if (steps.app?.sendsByItself ?? false)
        l10n.handoffStepSentByItself(site)
      else
        l10n.handoffStepPressEnter(site),
    ] else if (steps.prefilled) ...<String>[
      l10n.handoffStepOpensFilled(site),
      l10n.handoffStepDragFiles,
      l10n.handoffStepSend,
    ] else ...<String>[
      l10n.handoffStepOpensCopied(site),
      l10n.handoffStepDragFiles,
      l10n.handoffStepPasteSend,
    ],
    if (summary) ...<String>[
      l10n.handoffStepCopyAnswer,
      l10n.handoffStepComeBack,
    ] else
      l10n.handoffStepAskThere,
  ];
  final bool? open = await showDialog<bool>(
    context: context,
    builder: (BuildContext context) => AlertDialog(
      title: Text(summary ? l10n.summarizeIn(site) : l10n.chatIn(site)),
      content: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 440),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: <Widget>[
            for (final (int index, String line) in lines.indexed)
              Padding(
                padding: const EdgeInsets.only(bottom: 12),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: <Widget>[
                    CircleAvatar(radius: 12, child: Text('${index + 1}')),
                    const SizedBox(width: 12),
                    Expanded(child: Text(line)),
                  ],
                ),
              ),
          ],
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(false),
          child: Text(l10n.cancel),
        ),
        FilledButton(
          onPressed: () => Navigator.of(context).pop(true),
          child: Text(l10n.openSite(site)),
        ),
      ],
    ),
  );
  return open ?? false;
}

/// Tells the owner what happened to a hand-off when Koalino comes back.
final class HandoffReturnListener extends StatefulWidget {
  /// Creates the listener; it must sit below the app's `ScaffoldMessenger`.
  const HandoffReturnListener(
      {required this.controller, required this.child, super.key});

  final HandoffController controller;
  final Widget child;

  @override
  State<HandoffReturnListener> createState() => _HandoffReturnListenerState();
}

/// Shows one snack bar per event.
final class _HandoffReturnListenerState extends State<HandoffReturnListener> {
  StreamSubscription<HandoffEvent>? _events;

  @override
  void initState() {
    super.initState();
    _events = widget.controller.events.listen(_show);
  }

  @override
  void dispose() {
    _events?.cancel();
    super.dispose();
  }

  void _show(HandoffEvent event) {
    if (!mounted) {
      return;
    }
    final ScaffoldMessengerState messenger = ScaffoldMessenger.of(context);
    messenger.hideCurrentSnackBar();
    switch (event) {
      case HandoffKept(:final String title, :final String appName):
        messenger.showSnackBar(SnackBar(
          duration: const Duration(seconds: 8),
          content: Text(l10n.summaryFromSavedTo(appName, title)),
          action: SnackBarAction(
            label: l10n.undo,
            onPressed: () async {
              final String? problem = await widget.controller.undo(event);
              messenger.showSnackBar(SnackBar(
                  content: Text(problem ?? l10n.theEarlierSummaryIsBack)));
            },
          ),
        ));
      case HandoffHint(:final String appName):
        messenger.showSnackBar(SnackBar(
          duration: const Duration(seconds: 6),
          content: Text(widget.controller.viaBrowser
              ? l10n.toKeepTheSummaryCopyTheWebsite(appName)
              : l10n.toKeepTheSummaryCopyS(appName)),
        ));
      case HandoffFailed(:final String message):
        messenger.showSnackBar(SnackBar(content: Text(message)));
    }
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
