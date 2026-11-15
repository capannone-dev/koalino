// Shows the offer to record a call above every screen (#111), and says when a recording stopped
// because its call ended.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/call_watch_controller.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

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
                  child: Text(l10n.isUsingTheMicrophoneRecordThis(app)),
                ),
                actions: <Widget>[
                  TextButton(
                    onPressed: widget.controller.dismissOffer,
                    child: Text(l10n.notNow),
                  ),
                  FilledButton.icon(
                    icon: const Icon(Icons.fiber_manual_record),
                    label: Text(l10n.record),
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
                title: Text(l10n.offerToRecordCalls),
                subtitle: Text(l10n.whenTeamsZoomMeetInA),
                value: controller.settings.offer,
                onChanged: (bool offer) => controller.setSettings(offer: offer),
              ),
              SwitchListTile(
                secondary: const Icon(Icons.call_end),
                title: Text(l10n.stopWhenTheCallEnds),
                subtitle: Text(l10n.onlyForRecordingsStartedFromThe),
                value: controller.settings.autoStop,
                onChanged: (bool autoStop) =>
                    controller.setSettings(autoStop: autoStop),
              ),
              SwitchListTile(
                secondary: const Icon(Icons.badge_outlined),
                title: Text(l10n.readSpeakerNamesFromTheCall),
                subtitle: Text(l10n.whileRecordingKoalinoReadsWhoTeams),
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
