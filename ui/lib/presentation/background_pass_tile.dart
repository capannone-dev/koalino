// The phone's setting that lets the after-meeting pass run while Koalino is closed (#87).
// Android runs the pass from a charging job, but with Power saving on it holds the jobs of every
// app not exempt from battery optimisation. The tile shows whether Koalino is exempt and, when it
// is not, asks Android once through the system dialog. It hides itself off Android.

import 'package:flutter/material.dart';
import 'package:note_taker_ui/infrastructure/background_core.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Finds the tile in tests.
const Key backgroundPassTileKey = ValueKey<String>('background-pass');

/// Shows and requests the battery exemption the background pass needs.
final class BackgroundPassTile extends StatefulWidget {
  /// Creates the tile over [bridge].
  const BackgroundPassTile({required this.bridge, super.key});

  /// Reads and requests the exemption.
  final BackgroundPassBridge bridge;

  @override
  State<BackgroundPassTile> createState() => _BackgroundPassTileState();
}

/// Reads the state when shown and again whenever the app comes back to the front, since the
/// system dialog answers outside the app.
final class _BackgroundPassTileState extends State<BackgroundPassTile> {
  /// Whether Koalino is exempt; null while unknown or where the setting does not exist.
  bool? _exempt;
  late final AppLifecycleListener _lifecycle =
      AppLifecycleListener(onResume: _refresh);

  @override
  void initState() {
    super.initState();
    _lifecycle;
    _refresh();
  }

  @override
  void dispose() {
    _lifecycle.dispose();
    super.dispose();
  }

  /// Reads the exemption. Side effects: Asks the host; rebuilds.
  Future<void> _refresh() async {
    final bool? exempt = await widget.bridge.isExempt();
    if (mounted) {
      setState(() => _exempt = exempt);
    }
  }

  /// Shows the system dialog when Koalino is not exempt yet, then reads the answer.
  Future<void> _request() async {
    if (_exempt ?? true) {
      return;
    }
    await widget.bridge.requestExemption();
    await _refresh();
  }

  @override
  Widget build(BuildContext context) {
    final bool? exempt = _exempt;
    if (exempt == null) {
      return const SizedBox.shrink();
    }
    return ListTile(
      key: backgroundPassTileKey,
      leading: Icon(exempt
          ? Icons.battery_charging_full_outlined
          : Icons.battery_alert_outlined),
      title: Text(l10n.backgroundPassTitle),
      subtitle:
          Text(exempt ? l10n.backgroundPassAllowed : l10n.backgroundPassAsk),
      onTap: exempt ? null : _request,
    );
  }
}
