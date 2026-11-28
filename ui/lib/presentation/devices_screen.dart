// Shows this device, pairing requests and codes, paired devices, and nearby devices, and
// forwards the owner's choices to `DevicesController` (Issue #43). It also explains why devices on
// other networks are not found (Issue #66) and offers pairing invites (Issue #65). Nearby devices
// show the name they tell, and this device can be renamed (Issue #121). Paired devices say whether
// they are online and when they last synced, and pulling the list down reloads it (Issue #153).

// ignore: deprecated_member_use
import 'package:clock/clock.dart';
import 'package:flutter/gestures.dart';
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
        appBar: AppBar(
          title: Text(l10n.devices),
          actions: <Widget>[
            // Pulling down does the same; this serves mice without a drag habit.
            IconButton(
              tooltip: l10n.refresh,
              icon: const Icon(Icons.refresh),
              onPressed: controller.refresh,
            ),
          ],
        ),
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
            return _PullToRefresh(
              onRefresh: controller.refresh,
              child: status.enabled
                  ? _DevicesList(controller: controller, status: status)
                  : const _SyncOffMessage(),
            );
          },
        ),
      );
}

/// Reloads the status when the owner pulls `child` down past its top (#153).
///
/// Mice and trackpads can pull too, so desktops behave like phones; `child` must be a scrollable
/// that can always scroll, so a pull works even when everything fits on screen.
final class _PullToRefresh extends StatelessWidget {
  const _PullToRefresh({required this.onRefresh, required this.child});

  /// Reloads the status; the spinner shows until it completes.
  final Future<void> Function() onRefresh;
  final Widget child;

  @override
  Widget build(BuildContext context) => ScrollConfiguration(
        behavior: ScrollConfiguration.of(context).copyWith(
          dragDevices: <PointerDeviceKind>{
            PointerDeviceKind.touch,
            PointerDeviceKind.mouse,
            PointerDeviceKind.trackpad,
            PointerDeviceKind.stylus,
          },
        ),
        child: RefreshIndicator(onRefresh: onRefresh, child: child),
      );
}

/// Explains that the P2P task is off.
final class _SyncOffMessage extends StatelessWidget {
  const _SyncOffMessage();

  @override
  Widget build(BuildContext context) => LayoutBuilder(
        builder: (BuildContext context, BoxConstraints constraints) =>
            SingleChildScrollView(
          physics: const AlwaysScrollableScrollPhysics(),
          child: ConstrainedBox(
            constraints: BoxConstraints(minHeight: constraints.maxHeight),
            child: Center(child: _syncOffContent(context)),
          ),
        ),
      );

  /// The icon and the two lines of explanation.
  Widget _syncOffContent(BuildContext context) => Padding(
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
    final int onlineCount = status.pairedDevices
        .where((PairedDevice device) => device.connected)
        .length;
    return ListView(
      physics: const AlwaysScrollableScrollPhysics(),
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
        _SectionTitle(
          l10n.pairedDevices,
          trailing: status.pairedDevices.isEmpty
              ? null
              : l10n.devicesOnlineCount(
                  onlineCount, status.pairedDevices.length),
        ),
        if (status.pairedDevices.isEmpty) _EmptyText(l10n.noPairedDevicesYet),
        for (final PairedDevice device in status.pairedDevices)
          ListTile(
            isThreeLine: true,
            leading: Icon(
              Icons.verified_user_outlined,
              color: device.connected
                  ? onlineColor(Theme.of(context).colorScheme)
                  : Theme.of(context).colorScheme.outline,
            ),
            title: Wrap(
              spacing: 8,
              runSpacing: 4,
              crossAxisAlignment: WrapCrossAlignment.center,
              children: <Widget>[
                Text(device.deviceName),
                _ConnectionBadge(online: device.connected),
              ],
            ),
            subtitle: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: <Widget>[
                Text(
                  describeLastSynced(device.lastSyncedAt, now: clock.now()),
                  style: textTheme.bodyMedium?.copyWith(
                      color: Theme.of(context).colorScheme.onSurface),
                ),
                // A device can be connected a moment before its manifest arrives.
                Text(l10n
                    .paired(
                        formatPairedDate(device.pairedAt),
                        switch (device.capabilities) {
                          final DeviceCapabilities capabilities =>
                            describeCapabilities(capabilities),
                          null when device.connected => '',
                          null => describeDisconnectedDevice(
                              hasRelay: status.network.hasRelay),
                        })
                    .trimRight()),
              ],
            ),
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
            // A device tells its name only while it is discoverable (#121). Without one it is
            // an unnamed device, told apart by its short peer ID; a named one shows the ID only
            // when another nearby device has the same name (#211).
            title: Text(status.nearbyDeviceNames[peerId] ?? l10n.unnamedDevice),
            subtitle: _needsPeerId(status.nearbyDeviceNames, peerId)
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

/// A section heading, with an optional note at its end such as `1 of 2 online`.
final class _SectionTitle extends StatelessWidget {
  const _SectionTitle(this.text, {this.trailing});

  final String text;

  /// Shown muted at the end of the heading; `null` shows nothing.
  final String? trailing;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 20, 16, 4),
      child: Row(
        children: <Widget>[
          Expanded(
            child: Text(text,
                style: theme.textTheme.titleSmall
                    ?.copyWith(color: theme.colorScheme.primary)),
          ),
          if (trailing case final String note)
            Text(note,
                style: theme.textTheme.labelMedium
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
        ],
      ),
    );
  }
}

/// A small pill reading `Online` (green dot) or `Offline` (grey dot) beside a device's name.
final class _ConnectionBadge extends StatelessWidget {
  const _ConnectionBadge({required this.online});

  /// Whether the device is connected right now.
  final bool online;

  @override
  Widget build(BuildContext context) {
    final ThemeData theme = Theme.of(context);
    final Color dotColor =
        online ? onlineColor(theme.colorScheme) : theme.colorScheme.outline;
    return DecoratedBox(
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(12),
        border: Border.all(color: dotColor),
      ),
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 2),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: <Widget>[
            Icon(Icons.circle, size: 8, color: dotColor),
            const SizedBox(width: 6),
            Text(
              online ? l10n.deviceOnline : l10n.deviceOffline,
              style: theme.textTheme.labelMedium?.copyWith(color: dotColor),
            ),
          ],
        ),
      ),
    );
  }
}

/// The colour that marks a connected device: green, readable on the theme's surface.
///
/// Inputs: The theme's colours. Outputs: A darker green on light themes, a lighter one on dark.
/// Side effects: None.
Color onlineColor(ColorScheme colorScheme) =>
    colorScheme.brightness == Brightness.dark
        ? const Color(0xFF81C784)
        : const Color(0xFF2E7D32);

/// Says when a paired device last synced, as `Last synced 5 minutes ago` (#153).
///
/// Inputs: The last sync time (UTC or local), `null` for never; and the current time, which tests
/// fix. Outputs: `Never synced`; `Synced just now` under a minute, or for a time ahead of `now`
/// (the other device's clock does not matter, but this one may have moved); minutes under an hour;
/// hours under a day; then the local date and time. Side effects: None.
String describeLastSynced(DateTime? lastSyncedAt, {required DateTime now}) {
  if (lastSyncedAt == null) {
    return l10n.neverSynced;
  }
  final Duration sinceSync = now.difference(lastSyncedAt);
  if (sinceSync < const Duration(minutes: 1)) {
    return l10n.syncedJustNow;
  }
  if (sinceSync < const Duration(hours: 1)) {
    return l10n.lastSyncedMinutesAgo(sinceSync.inMinutes);
  }
  if (sinceSync < const Duration(days: 1)) {
    return l10n.lastSyncedHoursAgo(sinceSync.inHours);
  }
  return l10n.lastSyncedOn(formatLastSyncedDate(lastSyncedAt));
}

/// Formats an older sync time in local time, as `20 Nov 2026, 09:05` in English.
///
/// Inputs: The sync time. Outputs: The date and 24-hour time in the app's language.
/// Side effects: None.
String formatLastSyncedDate(DateTime lastSyncedAt) =>
    formatDatePattern(lastSyncedAt.toLocal(), 'd MMM y, HH:mm');

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

/// Whether a nearby device's short peer ID must show to tell it apart (#211).
///
/// Inputs: the names nearby devices told, by peer ID; the device's `peerId`.
/// Outputs: true when the device has no name, or shares its name with another nearby device.
/// Side effects: none.
bool _needsPeerId(Map<String, String> names, String peerId) {
  final String? name = names[peerId];
  if (name == null) return true;
  return names.entries.any((MapEntry<String, String> other) =>
      other.key != peerId && other.value == name);
}
