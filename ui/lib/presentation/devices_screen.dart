// Shows this device, pairing requests and codes, paired devices, and nearby devices, and
// forwards the owner's choices to `DevicesController` (Issue #43).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';

/// The Devices screen; it renders the controller's state and holds no logic of its own.
///
/// The composition root starts and stops the controller's polling, so this widget can be pumped
/// in tests over a controller that was refreshed once.
final class DevicesScreen extends StatelessWidget {
  /// Creates the screen over `controller`.
  const DevicesScreen({required this.controller, super.key});

  /// Supplies the status and runs every action.
  final DevicesController controller;

  @override
  Widget build(BuildContext context) => Scaffold(
        appBar: AppBar(title: const Text('Devices')),
        body: ListenableBuilder(
          listenable: controller,
          builder: (BuildContext context, Widget? child) {
            final P2pStatus? status = controller.status;
            if (status == null) {
              return Center(
                child: controller.errorMessage == null
                    ? const CircularProgressIndicator()
                    : Text(controller.errorMessage!),
              );
            }
            if (!status.enabled) {
              return const _SyncOffMessage();
            }
            return _DevicesList(controller: controller, status: status);
          },
        ),
      );
}

/// Explains that the P2P task is off.
final class _SyncOffMessage extends StatelessWidget {
  const _SyncOffMessage();

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.all(24),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              Icon(Icons.sync_disabled,
                  size: 48, color: Theme.of(context).colorScheme.outline),
              const SizedBox(height: 16),
              Text('Device sync is off',
                  style: Theme.of(context).textTheme.titleLarge),
              const SizedBox(height: 8),
              const Text(
                'Meetings stay on this device until sync is turned on.',
                textAlign: TextAlign.center,
              ),
            ],
          ),
        ),
      );
}

/// The scrolling body for a running P2P task.
final class _DevicesList extends StatelessWidget {
  const _DevicesList({required this.controller, required this.status});

  final DevicesController controller;
  final P2pStatus status;

  @override
  Widget build(BuildContext context) {
    final TextTheme textTheme = Theme.of(context).textTheme;
    return ListView(
      padding: const EdgeInsets.symmetric(vertical: 8),
      children: <Widget>[
        if (controller.errorMessage case final String message)
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
            child: Text(message,
                style: TextStyle(color: Theme.of(context).colorScheme.error)),
          ),
        ListTile(
          leading: const Icon(Icons.devices),
          title: Text(status.deviceName),
          subtitle: const Text('This device'),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: controller.discoverable
              ? Text(
                  'Discoverable: your other devices can ask to pair for a few minutes.',
                  style: textTheme.bodyMedium,
                )
              : Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: OutlinedButton.icon(
                    onPressed: controller.makeDiscoverable,
                    icon: const Icon(Icons.wifi_tethering),
                    label: const Text('Make discoverable'),
                  ),
                ),
        ),
        if (status.incomingRequests.isNotEmpty) ...<Widget>[
          const _SectionTitle('Pairing requests'),
          for (final IncomingPairingRequest request in status.incomingRequests)
            _CodeCard(
              title: request.deviceName,
              code: request.code,
              instructions:
                  'Accept only if the other device shows the same code.',
              actions: <Widget>[
                TextButton(
                  onPressed: () => controller.rejectRequest(request.peerId),
                  child: const Text('Reject'),
                ),
                FilledButton(
                  onPressed: () => controller.acceptRequest(request.peerId),
                  child: const Text('Accept'),
                ),
              ],
            ),
        ],
        if (status.outgoingPairings.isNotEmpty) ...<Widget>[
          const _SectionTitle('Pairing in progress'),
          for (final OutgoingPairing pairing in status.outgoingPairings)
            _CodeCard(
              title: shortPeerId(pairing.peerId),
              code: pairing.code,
              instructions: pairing.locallyConfirmed
                  ? 'Waiting for the other device…'
                  : 'Check that the other device shows the same code.',
              actions: <Widget>[
                TextButton(
                  onPressed: () => controller.cancelPairing(pairing.peerId),
                  child: const Text('Cancel pairing'),
                ),
                if (!pairing.locallyConfirmed)
                  FilledButton(
                    onPressed: () => controller.confirmCode(pairing.peerId),
                    child: const Text('Codes match'),
                  ),
              ],
            ),
        ],
        const _SectionTitle('Paired devices'),
        if (status.pairedDevices.isEmpty)
          const _EmptyText('No paired devices yet.'),
        for (final PairedDevice device in status.pairedDevices)
          ListTile(
            leading: const Icon(Icons.verified_user_outlined),
            title: Text(device.deviceName),
            subtitle: Text('Paired ${formatPairedDate(device.pairedAt)}'),
            trailing: IconButton(
              tooltip: 'Remove ${device.deviceName}',
              icon: const Icon(Icons.delete_outline),
              onPressed: () => _confirmRemoval(context, device),
            ),
          ),
        const _SectionTitle('Nearby devices'),
        if (status.nearbyPeerIds.isEmpty)
          const _EmptyText(
              'No other devices found. Open Note Taker on another device on this network '
              'and make it discoverable.'),
        for (final String peerId in status.nearbyPeerIds)
          ListTile(
            leading: const Icon(Icons.wifi),
            title: Text(shortPeerId(peerId)),
            trailing: FilledButton.tonal(
              onPressed: () => controller.startPairing(peerId),
              child: const Text('Pair'),
            ),
          ),
      ],
    );
  }

  /// Asks before removing `device`, then removes it on confirmation.
  Future<void> _confirmRemoval(
      BuildContext context, PairedDevice device) async {
    final bool? confirmed = await showDialog<bool>(
      context: context,
      builder: (BuildContext dialogContext) => AlertDialog(
        title: Text('Remove ${device.deviceName}?'),
        content: Text(
            '${device.deviceName} stops syncing with this device. Pair again to resume.'),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Remove'),
          ),
        ],
      ),
    );
    if (confirmed ?? false) {
      await controller.removeDevice(device.peerId);
    }
  }
}

/// A card with a device name, a pairing code in large type, and its actions.
final class _CodeCard extends StatelessWidget {
  const _CodeCard({
    required this.title,
    required this.code,
    required this.instructions,
    required this.actions,
  });

  final String title;
  final String code;
  final String instructions;
  final List<Widget> actions;

  @override
  Widget build(BuildContext context) {
    final TextTheme textTheme = Theme.of(context).textTheme;
    return Card(
      margin: const EdgeInsets.symmetric(horizontal: 16, vertical: 6),
      child: Padding(
        padding: const EdgeInsets.fromLTRB(16, 12, 16, 4),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: <Widget>[
            Text(title, style: textTheme.titleMedium),
            Semantics(
              label: 'Pairing code ${code.split('').join(' ')}',
              excludeSemantics: true,
              child: Text(
                formatPairingCode(code),
                style: textTheme.headlineMedium?.copyWith(
                  fontFeatures: const <FontFeature>[
                    FontFeature.tabularFigures()
                  ],
                  letterSpacing: 2,
                ),
              ),
            ),
            Text(instructions, style: textTheme.bodyMedium),
            OverflowBar(
              alignment: MainAxisAlignment.end,
              spacing: 8,
              children: actions,
            ),
          ],
        ),
      ),
    );
  }
}

/// A section heading.
final class _SectionTitle extends StatelessWidget {
  const _SectionTitle(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.fromLTRB(16, 20, 16, 4),
        child: Text(text,
            style: Theme.of(context)
                .textTheme
                .titleSmall
                ?.copyWith(color: Theme.of(context).colorScheme.primary)),
      );
}

/// Muted text for an empty section.
final class _EmptyText extends StatelessWidget {
  const _EmptyText(this.text);

  final String text;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
        child: Text(text,
            style: TextStyle(color: Theme.of(context).colorScheme.outline)),
      );
}

/// Splits a six-digit code into two groups of three, such as `639 210`, for reading aloud.
///
/// Inputs: The code from Rust. Outputs: The grouped code, or the input unchanged when it is not
/// six characters long. Side effects: None.
String formatPairingCode(String code) =>
    code.length == 6 ? '${code.substring(0, 3)} ${code.substring(3)}' : code;

/// Shortens a peer ID for display as `12D3Koo…1234`; devices are told apart by their tail.
///
/// Inputs: A libp2p peer ID. Outputs: The shortened ID, or the input when it is already short.
/// Side effects: None.
String shortPeerId(String peerId) => peerId.length <= 16
    ? peerId
    : '${peerId.substring(0, 7)}…${peerId.substring(peerId.length - 6)}';

/// Formats a pairing date in local time as `28 Oct 2026`.
///
/// Inputs: The UTC pairing time. Outputs: The day, month name, and year. Side effects: None.
String formatPairedDate(DateTime pairedAt) {
  const List<String> monthNames = <String>[
    'Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', //
    'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec',
  ];
  final DateTime local = pairedAt.toLocal();
  return '${local.day} ${monthNames[local.month - 1]} ${local.year}';
}
