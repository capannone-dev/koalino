// Covers the app with the lock screen while `AppLockController` says it is locked. The app's
// widgets stay mounted (only hidden), so an active recording and every screen keep their state.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/app_lock_controller.dart';

/// Shows `child` only while the app is unlocked, and reports app visibility to the controller.
final class AppLockGate extends StatefulWidget {
  /// Creates the gate.
  ///
  /// Inputs: The controller (the composition root initializes it) and the app.
  /// Outputs: The lock screen, a blank screen while the setting is read, or the app.
  /// Side effects: Shows the system prompt on its own when the app locks while in view.
  const AppLockGate({required this.controller, required this.child, super.key});

  final AppLockController controller;
  final Widget child;

  @override
  State<AppLockGate> createState() => _AppLockGateState();
}

/// Forwards lifecycle changes and starts the automatic prompt.
final class _AppLockGateState extends State<AppLockGate> {
  late final AppLifecycleListener _lifecycleListener = AppLifecycleListener(
    onHide: widget.controller.appHidden,
    onShow: () {
      widget.controller.appShown();
      _promptWhenInView();
    },
  );

  @override
  void initState() {
    super.initState();
    _lifecycleListener;
    widget.controller.addListener(_promptWhenInView);
  }

  @override
  void dispose() {
    widget.controller.removeListener(_promptWhenInView);
    _lifecycleListener.dispose();
    super.dispose();
  }

  /// Shows the prompt after this frame when the app locked while in view; Android refuses a
  /// prompt for an activity in the background.
  void _promptWhenInView() {
    final AppLifecycleState? lifecycleState =
        WidgetsBinding.instance.lifecycleState;
    if (widget.controller.phase != AppLockPhase.locked ||
        (lifecycleState != null &&
            lifecycleState != AppLifecycleState.resumed)) {
      return;
    }
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) {
        widget.controller.promptIfPending();
      }
    });
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: widget.controller,
        builder: (BuildContext context, Widget? app) {
          final AppLockPhase phase = widget.controller.phase;
          return Stack(
            fit: StackFit.expand,
            children: <Widget>[
              // Not painted, hit-tested, or read by screen readers while locked.
              Visibility(
                visible: phase == AppLockPhase.unlocked,
                maintainState: true,
                child: app!,
              ),
              if (phase == AppLockPhase.locked)
                _LockScreen(controller: widget.controller),
              if (phase == AppLockPhase.checking)
                ColoredBox(color: Theme.of(context).colorScheme.surface),
            ],
          );
        },
        child: widget.child,
      );
}

/// The lock screen: the app's name, why it is locked, and Unlock.
final class _LockScreen extends StatelessWidget {
  const _LockScreen({required this.controller});

  final AppLockController controller;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Scaffold(
      body: SafeArea(
        child: Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: <Widget>[
                Icon(Icons.lock_outline,
                    size: 56, color: theme.colorScheme.primary),
                const SizedBox(height: 16),
                Text('Koalino is locked', style: theme.textTheme.headlineSmall),
                const SizedBox(height: 8),
                const Text(
                  'Unlock with your fingerprint, face, or screen lock.',
                  textAlign: TextAlign.center,
                ),
                if (controller.message case final String message) ...<Widget>[
                  const SizedBox(height: 12),
                  Text(message,
                      textAlign: TextAlign.center,
                      style: TextStyle(color: theme.colorScheme.error)),
                ],
                const SizedBox(height: 24),
                FilledButton.icon(
                  onPressed:
                      controller.isAuthenticating ? null : controller.unlock,
                  icon: const Icon(Icons.fingerprint),
                  label: const Text('Unlock'),
                ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}
