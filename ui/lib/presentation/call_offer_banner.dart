// Shows the offer to record a call above every screen (#111), and says when a recording stopped
// because its call ended.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/call_watch_controller.dart';

/// Puts the call offer above `child`.
final class CallOfferBanner extends StatefulWidget {
  /// Creates the banner over the app's screens.
  const CallOfferBanner(
      {required this.controller, required this.child, super.key});

  final CallWatchController controller;
  final Widget child;

  @override
  State<CallOfferBanner> createState() => _CallOfferBannerState();
}

final class _CallOfferBannerState extends State<CallOfferBanner> {
  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_changed);
  }

  @override
  void dispose() {
    widget.controller.removeListener(_changed);
    super.dispose();
  }

  void _changed() {
    if (!mounted) {
      return;
    }
    if (widget.controller.message case final String message) {
      widget.controller.clearMessage();
      ScaffoldMessenger.maybeOf(context)
          ?.showSnackBar(SnackBar(content: Text(message)));
    }
    setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final String? app = widget.controller.offeredApp;
    return Column(
      children: <Widget>[
        if (app != null)
          Material(
            child: SafeArea(
              bottom: false,
              child: MaterialBanner(
                leading: const Icon(Icons.call),
                content: Semantics(
                  liveRegion: true,
                  child:
                      Text('$app is using the microphone. Record this call?'),
                ),
                actions: <Widget>[
                  TextButton(
                    onPressed: widget.controller.dismissOffer,
                    child: const Text('Not now'),
                  ),
                  FilledButton.icon(
                    icon: const Icon(Icons.fiber_manual_record),
                    label: const Text('Record'),
                    onPressed: widget.controller.acceptOffer,
                  ),
                ],
              ),
            ),
          ),
        Expanded(child: widget.child),
      ],
    );
  }
}

/// The call watch's two Settings switches (#111), shown where the system can tell.
final class CallWatchSettingsTiles extends StatelessWidget {
  /// Creates the switches.
  const CallWatchSettingsTiles({required this.controller, super.key});

  final CallWatchController controller;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: controller,
        builder: (BuildContext context, Widget? _) {
          if (!controller.available) {
            return const SizedBox.shrink();
          }
          return Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              SwitchListTile(
                secondary: const Icon(Icons.call),
                title: const Text('Offer to record calls'),
                subtitle: const Text(
                    'When Teams, Zoom, Meet in a browser or another call app starts using '
                    'the microphone. Nothing records until you tap Record.'),
                value: controller.settings.offer,
                onChanged: (bool offer) => controller.setSettings(offer: offer),
              ),
              SwitchListTile(
                secondary: const Icon(Icons.call_end),
                title: const Text('Stop when the call ends'),
                subtitle: const Text(
                    'Only for recordings started from the offer, once the call app has left '
                    'the microphone for 20 seconds.'),
                value: controller.settings.autoStop,
                onChanged: (bool autoStop) =>
                    controller.setSettings(autoStop: autoStop),
              ),
              SwitchListTile(
                secondary: const Icon(Icons.badge_outlined),
                title: const Text('Read speaker names from the call window'),
                subtitle: const Text(
                    'While recording, Koalino reads who Teams, Zoom or Meet shows as '
                    'speaking, to suggest names for unknown voices. Nothing leaves this '
                    'computer, and nothing is named until you tap Assign.'),
                value: controller.settings.readNames,
                onChanged: (bool readNames) =>
                    controller.setSettings(readNames: readNames),
              ),
              const Divider(),
            ],
          );
        },
      );
}
