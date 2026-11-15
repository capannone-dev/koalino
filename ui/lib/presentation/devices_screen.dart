// Shows this device, pairing requests and codes, paired devices, and nearby devices, and
// forwards the owner's choices to `DevicesController` (Issue #43). It also explains why devices on
// other networks are not found (Issue #66) and offers pairing invites (Issue #65). Nearby devices
// show the name they tell, and this device can be renamed (Issue #121).

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/presentation/pairing_invite_dialogs.dart';
import 'package:note_taker_ui/presentation/rename_device_dialog.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

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
        appBar: AppBar(title: Text(l10n.devices)),
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
              Text(l10n.deviceSyncIsOff,
                  style: Theme.of(context).textTheme.titleLarge),
              const SizedBox(height: 8),
              Text(
                l10n.meetingsStayOnThisDeviceUntil,
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
          subtitle: Text(switch (status.capabilities) {
            final DeviceCapabilities capabilities =>
              l10n.thisDevice3(describeCapabilities(capabilities)),
            null => l10n.thisDevice,
          }),
          trailing: IconButton(
            tooltip: l10n.renameThisDevice,
            icon: const Icon(Icons.edit_outlined),
            onPressed: () => showRenameDeviceDialog(context,
                controller: controller, currentName: status.deviceName),
          ),
        ),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: controller.discoverable
              ? Text(
                  l10n.discoverableYourOtherDevicesCanAsk,
                  style: textTheme.bodyMedium,
                )
              : Align(
                  alignment: AlignmentDirectional.centerStart,
                  child: OutlinedButton.icon(
                    onPressed: controller.makeDiscoverable,
                    icon: const Icon(Icons.wifi_tethering),
                    label: Text(l10n.makeDiscoverable),
                  ),
                ),
        ),
        if (status.incomingRequests.isNotEmpty) ...<Widget>[
          _SectionTitle(l10n.pairingRequests),
          for (final IncomingPairingRequest request in status.incomingRequests)
            _CodeCard(
              title: request.deviceName,
              code: request.code,
              instructions: l10n.acceptOnlyIfTheOtherDevice,
              actions: <Widget>[
                TextButton(
                  onPressed: () => controller.rejectRequest(request.peerId),
                  child: Text(l10n.reject),
                ),
                FilledButton(
                  onPressed: () => controller.acceptRequest(request.peerId),
                  child: Text(l10n.accept),
                ),
              ],
            ),
        ],
        if (status.outgoingPairings.isNotEmpty) ...<Widget>[
          _SectionTitle(l10n.pairingInProgress),
          for (final OutgoingPairing pairing in status.outgoingPairings)
            _CodeCard(
              title: shortPeerId(pairing.peerId),
              code: pairing.code,
              instructions: pairing.locallyConfirmed
                  ? l10n.waitingForTheOtherDevice
                  : l10n.checkThatTheOtherDeviceShows,
              actions: <Widget>[
                TextButton(
                  onPressed: () => controller.cancelPairing(pairing.peerId),
                  child: Text(l10n.cancelPairing),
                ),
                if (!pairing.locallyConfirmed)
                  FilledButton(
                    onPressed: () => controller.confirmCode(pairing.peerId),
                    child: Text(l10n.codesMatch),
                  ),
              ],
            ),
        ],
        _SectionTitle(l10n.pairedDevices),
        if (status.pairedDevices.isEmpty) _EmptyText(l10n.noPairedDevicesYet),
        for (final PairedDevice device in status.pairedDevices)
          ListTile(
            leading: const Icon(Icons.verified_user_outlined),
            title: Text(device.deviceName),
            subtitle: Text(l10n.paired(
                formatPairedDate(device.pairedAt),
                switch (device.capabilities) {
                  final DeviceCapabilities capabilities =>
                    describeCapabilities(capabilities),
                  null => describeDisconnectedDevice(
                      hasRelay: status.network.hasRelay),
                })),
            trailing: IconButton(
              tooltip: l10n.remove3(device.deviceName),
              icon: const Icon(Icons.delete_outline),
              onPressed: () => _confirmRemoval(context, device),
            ),
          ),
        _SectionTitle(l10n.nearbyDevices),
        if (status.nearbyPeerIds.isEmpty)
          _EmptyText(describeEmptyNearby(hasRelay: status.network.hasRelay)),
        for (final String peerId in status.nearbyPeerIds)
          ListTile(
            leading: const Icon(Icons.wifi),
            // A device tells its name only while it is discoverable (#121).
            title:
                Text(status.nearbyDeviceNames[peerId] ?? shortPeerId(peerId)),
            subtitle: status.nearbyDeviceNames.containsKey(peerId)
                ? Text(shortPeerId(peerId))
                : null,
            trailing: FilledButton.tonal(
              onPressed: () => controller.startPairing(peerId),
              child: Text(l10n.pair),
            ),
          ),
        _SectionTitle(l10n.anotherNetwork),
        _EmptyText(l10n.showAnInviteHereAndUse),
        Padding(
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          child: Wrap(
            spacing: 8,
            runSpacing: 8,
            children: <Widget>[
              OutlinedButton.icon(
                onPressed: () => showInviteDialog(context, controller),
                icon: const Icon(Icons.qr_code_2),
                label: Text(l10n.showInvite),
              ),
              OutlinedButton.icon(
                onPressed: () => showJoinInviteDialog(context, controller),
                icon: const Icon(Icons.link),
                label: Text(l10n.useInvite),
              ),
            ],
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
        title: Text(l10n.remove(device.deviceName)),
        content: Text(l10n.stopsSyncingWithThisDevicePair(device.deviceName)),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: Text(l10n.remove2),
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
              label: l10n.pairingCode(code.split('').join(' ')),
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

/// Explains an empty Nearby list: it only covers this network, and what to do otherwise.
///
/// Inputs: Whether a relay is set. Outputs: The message. Side effects: None.
String describeEmptyNearby({required bool hasRelay}) =>
    hasRelay ? l10n.noOtherDevicesFoundOnThis : l10n.noOtherDevicesFoundOnThis2;

/// Explains what a paired device that is not connected needs to reconnect.
///
/// Inputs: Whether a relay is set. Outputs: The message. Side effects: None.
String describeDisconnectedDevice({required bool hasRelay}) => hasRelay
    ? l10n.notConnectedItReconnectsWhenIt
    : l10n.notConnectedPutBothDevicesOn;

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

/// Display names of known transcription model labels; others are shown as given.
Map<String, String> get _modelNames => <String, String>{
      'whisper': l10n.whisperOnDevice,
      'openai': l10n.openaiCloud,
    };

/// Summarizes a device's manifest as `8 CPU cores · 4 GB GPU · Whisper (on device)`.
///
/// Inputs: The advertised capabilities. Outputs: One line of text. Side effects: None.
String describeCapabilities(DeviceCapabilities capabilities) {
  final List<String> parts = <String>[
    l10n.cpuCores(capabilities.cpuCores),
  ];
  final int gpuMegabytes = capabilities.gpuVramMegabytes;
  if (gpuMegabytes > 0) {
    parts.add(gpuMegabytes >= 1024
        ? l10n.gbGpu((gpuMegabytes / 1024).round())
        : l10n.mbGpu(gpuMegabytes));
  }
  parts.add(capabilities.transcriptionModels.isEmpty
      ? l10n.noTranscriptionModels
      : capabilities.transcriptionModels
          .map((String model) => _modelNames[model] ?? model)
          .join(', '));
  return parts.join(' · ');
}

/// Formats a pairing date in local time, as `28 Oct 2026` in English.
///
/// Inputs: The UTC pairing time. Outputs: The day, abbreviated month and year in the app's
/// language (#124). Side effects: None.
String formatPairedDate(DateTime pairedAt) =>
    formatDatePattern(pairedAt.toLocal(), 'd MMM y');
