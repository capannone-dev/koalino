// Shows the app only once the Rust core has started; until then a waiting message, and after a
// failure an explanation with Retry and Quit.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/core_startup_controller.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Replaces `child` with start-up feedback until `controller` is `ready`.
final class CoreStartupGate extends StatelessWidget {
  /// Creates the gate.
  ///
  /// Inputs: The controller (its `start` is called by the composition root), the platform's
  /// quit action, and the app to show once the core is open.
  const CoreStartupGate({
    required this.controller,
    required this.onQuit,
    required this.child,
    super.key,
  });

  final CoreStartupController controller;
  final VoidCallback onQuit;
  final Widget child;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: controller,
        builder: (BuildContext context, Widget? _) =>
            switch (controller.state) {
          CoreStartupState.ready => child,
          CoreStartupState.starting => const _StartingMessage(),
          CoreStartupState.keychainUnavailable => _StartupFailure(
              title: l10n.unableToOpenTheKeychain,
              explanation: l10n.koalinoKeepsTheKeyThatEncrypts,
              onRetry: controller.start,
              onQuit: onQuit,
            ),
          CoreStartupState.failed => _StartupFailure(
              title: l10n.unableToOpenYourNotes,
              explanation: l10n.koalinoCouldNotOpenItsDatabase,
              onRetry: controller.start,
              onQuit: onQuit,
            ),
        },
      );
}

/// Shown while the core opens, which can wait on a keychain unlock prompt.
final class _StartingMessage extends StatelessWidget {
  const _StartingMessage();

  @override
  Widget build(BuildContext context) => Scaffold(
        body: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              CircularProgressIndicator(),
              SizedBox(height: 16),
              Text(l10n.unlockingYourNotes),
            ],
          ),
        ),
      );
}

/// A start-up failure with Retry and Quit.
final class _StartupFailure extends StatelessWidget {
  const _StartupFailure({
    required this.title,
    required this.explanation,
    required this.onRetry,
    required this.onQuit,
  });

  final String title;
  final String explanation;
  final VoidCallback onRetry;
  final VoidCallback onQuit;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Scaffold(
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 480),
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Icon(Icons.lock_outline,
                    size: 48, color: theme.colorScheme.error),
                const SizedBox(height: 16),
                Semantics(
                  header: true,
                  child: Text(title, style: theme.textTheme.headlineSmall),
                ),
                const SizedBox(height: 12),
                Text(explanation, style: theme.textTheme.bodyLarge),
                const SizedBox(height: 24),
                OverflowBar(
                  alignment: MainAxisAlignment.end,
                  spacing: 8,
                  children: <Widget>[
                    TextButton(onPressed: onQuit, child: Text(l10n.quit)),
                    FilledButton(onPressed: onRetry, child: Text(l10n.retry)),
                  ],
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
