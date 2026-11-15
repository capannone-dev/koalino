// Starts pairing from an invite link a phone's camera opened (Issue #122). The owner is asked
// first, then the Devices screen opens with the pairing under way, where both devices show the
// code to compare. Opening a link never pairs by itself.

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/pairing_link_bridge.dart';
import 'package:note_taker_ui/presentation/devices_screen.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Builds the controller the Devices screen opened by a link uses; tests give a fake bridge.
typedef DevicesControllerFactory = DevicesController Function();

/// Watches for invite links and offers to pair with the device that made each one.
final class PairingLinkListener extends StatefulWidget {
  /// Creates the listener around `child`.
  const PairingLinkListener({
    required this.linkBridge,
    required this.devicesController,
    required this.child,
    super.key,
  });

  /// Where links come from.
  final PairingLinkBridge linkBridge;

  /// Makes the controller for the Devices screen a link opens.
  final DevicesControllerFactory devicesController;

  /// The app below.
  final Widget child;

  @override
  State<PairingLinkListener> createState() => _PairingLinkListenerState();
}

/// Handles one link at a time; a link arriving meanwhile waits as Kotlin's pending one.
final class _PairingLinkListenerState extends State<PairingLinkListener> {
  StreamSubscription<void>? _arrivals;
  bool _isHandling = false;

  @override
  void initState() {
    super.initState();
    _arrivals = widget.linkBridge.arrivals.listen((_) => _takeLink());
    // After the first frame, so the question has a navigator to show on.
    WidgetsBinding.instance.addPostFrameCallback((_) => _takeLink());
  }

  @override
  void dispose() {
    _arrivals?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) => widget.child;

  /// Takes the pending link, if any, and handles it; then looks for one that came meanwhile.
  Future<void> _takeLink() async {
    if (_isHandling || !mounted) {
      return;
    }
    _isHandling = true;
    try {
      String? link = await widget.linkBridge.takePending();
      while (link != null && mounted) {
        await _handle(link);
        link = await widget.linkBridge.takePending();
      }
    } finally {
      _isHandling = false;
    }
  }

  /// Asks the owner, then opens the Devices screen and starts pairing there.
  ///
  /// Side effects: On Pair, dials the inviting device and pushes the Devices screen; a refusal
  /// is shown as a snack bar on that screen.
  Future<void> _handle(String link) async {
    final bool? pair = await showDialog<bool>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: Text(l10n.pairWithAnotherDevice),
        content: Text(l10n.youOpenedAPairingInviteKoalino),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.pair),
          ),
        ],
      ),
    );
    if (pair != true || !mounted) {
      return;
    }
    final DevicesController controller = widget.devicesController();
    controller.startPolling();
    final Future<void> closed = Navigator.of(context).push(
      MaterialPageRoute<void>(
        builder: (BuildContext pageContext) =>
            _DevicesPairingPage(controller: controller, link: link),
      ),
    );
    await closed;
    controller.dispose();
  }
}

/// The Devices screen, starting the pairing once it is shown.
final class _DevicesPairingPage extends StatefulWidget {
  const _DevicesPairingPage({required this.controller, required this.link});

  final DevicesController controller;
  final String link;

  @override
  State<_DevicesPairingPage> createState() => _DevicesPairingPageState();
}

/// Starts the pairing after the first frame, so a refusal has a scaffold to show on.
final class _DevicesPairingPageState extends State<_DevicesPairingPage> {
  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) => _start());
  }

  Future<void> _start() async {
    final String? refusal = await widget.controller.pairWithInvite(widget.link);
    if (!mounted) {
      return;
    }
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(
        content: Text(refusal ?? l10n.pairingStartedCheckThatBothDevices)));
  }

  @override
  Widget build(BuildContext context) =>
      DevicesScreen(controller: widget.controller);
}
