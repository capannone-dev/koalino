// The Settings destination. It holds the Android App lock switch for now; the logs and other
// settings of Issue #46 join it later.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/app_lock_controller.dart';
import 'package:note_taker_ui/infrastructure/app_lock_bridge.dart';

/// Renders the settings over their controllers; it holds no logic of its own.
final class SettingsScreen extends StatelessWidget {
  /// Creates the screen.
  ///
  /// Inputs: The app-lock controller, already initialized by the composition root.
  /// Outputs: A Material 3 settings list.
  /// Side effects: None; the switch asks the controller, which shows the system prompt.
  const SettingsScreen({required this.appLockController, super.key});

  final AppLockController appLockController;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Settings')),
        body: ListenableBuilder(
          listenable: appLockController,
          builder: (BuildContext context, Widget? child) {
            final AppLockStatus? status = appLockController.status;
            final bool available = status != null &&
                status.supported &&
                status.deviceSecure &&
                !appLockController.isAuthenticating;
            return ListView(
              children: <Widget>[
                SwitchListTile(
                  secondary: const Icon(Icons.lock_outline),
                  title: const Text('App lock'),
                  subtitle: Text(_appLockDescription(status)),
                  value: status?.locks ?? false,
                  onChanged: available ? appLockController.setEnabled : null,
                ),
                if (appLockController.message case final String message)
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 16),
                    child: Text(message,
                        style: TextStyle(
                            color: Theme.of(context).colorScheme.error)),
                  ),
              ],
            );
          },
        ),
      );

  /// Explains what the switch does, or why it is unavailable.
  static String _appLockDescription(AppLockStatus? status) => switch (status) {
        null => 'Unable to read the app lock setting.',
        AppLockStatus(supported: false) => 'Needs Android 10 or later.',
        AppLockStatus(deviceSecure: false) =>
          'Set a screen lock in Android settings to use App lock.',
        _ => 'Ask for your fingerprint, face, or screen lock when you return '
            'to Note Taker after ${lockAfterBackground.inSeconds} seconds '
            'away. Recording and sync keep running while it is locked.',
      };
}
