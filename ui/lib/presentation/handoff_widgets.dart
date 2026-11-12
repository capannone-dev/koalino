// Hands a meeting to the owner's own AI app (#114) on Android. The sheet offers Summarize and Chat
// in the Claude or ChatGPT app when installed, or in any app through the share sheet. The listener
// around the app's screens tells the owner when an answer was kept as a summary, with Undo, or
// how to keep it.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/handoff_controller.dart';
import 'package:note_taker_ui/infrastructure/handoff_bridge.dart';

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
  final _HandoffChoice? choice = await showModalBottomSheet<_HandoffChoice>(
    context: context,
    showDragHandle: true,
    builder: (BuildContext context) => SafeArea(
      child: ListView(
        shrinkWrap: true,
        children: <Widget>[
          Padding(
            padding: const EdgeInsets.fromLTRB(24, 0, 24, 8),
            child: Text('Use your own AI app',
                style: Theme.of(context).textTheme.titleLarge),
          ),
          const Padding(
            padding: EdgeInsets.fromLTRB(24, 0, 24, 8),
            child: Text(
                'The meeting, its notes and its files go to the app you pick, '
                'with your own account. For a summary, copy the answer there and '
                'come back: Koalino keeps it.'),
          ),
          for (final AiApp app in controller.apps) ...<Widget>[
            ListTile(
              leading: const Icon(Icons.summarize_outlined),
              title: Text('Summarize in ${app.displayName}'),
              onTap: () => Navigator.of(context)
                  .pop(_HandoffChoice(HandoffPurpose.summary, app)),
            ),
            ListTile(
              leading: const Icon(Icons.forum_outlined),
              title: Text('Chat in ${app.displayName}'),
              onTap: () => Navigator.of(context)
                  .pop(_HandoffChoice(HandoffPurpose.chat, app)),
            ),
          ],
          ListTile(
            leading: const Icon(Icons.share_outlined),
            title: const Text('Summarize in another app…'),
            onTap: () => Navigator.of(context)
                .pop(const _HandoffChoice(HandoffPurpose.summary, null)),
          ),
          ListTile(
            leading: const Icon(Icons.share_outlined),
            title: const Text('Chat in another app…'),
            onTap: () => Navigator.of(context)
                .pop(const _HandoffChoice(HandoffPurpose.chat, null)),
          ),
        ],
      ),
    ),
  );
  if (choice == null) {
    return;
  }
  final String? problem = await controller
      .handOff(meetingId, title, choice.purpose, app: choice.app);
  if (problem != null) {
    messenger.showSnackBar(SnackBar(content: Text(problem)));
  }
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
          content: Text('Summary from $appName saved to "$title".'),
          action: SnackBarAction(
            label: 'Undo',
            onPressed: () async {
              final String? problem = await widget.controller.undo(event);
              messenger.showSnackBar(SnackBar(
                  content: Text(problem ?? 'The earlier summary is back.')));
            },
          ),
        ));
      case HandoffHint(:final String appName):
        messenger.showSnackBar(SnackBar(
          duration: const Duration(seconds: 6),
          content: Text(
              'To keep the summary, copy $appName\'s answer, then come back to Koalino.'),
        ));
      case HandoffFailed(:final String message):
        messenger.showSnackBar(SnackBar(content: Text(message)));
    }
  }

  @override
  Widget build(BuildContext context) => widget.child;
}
