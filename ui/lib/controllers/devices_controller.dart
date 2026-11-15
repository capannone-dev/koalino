// Owns the Devices screen's state: the P2P status, pairing commands, and readable errors
// (Issue #43). Widgets call it; it is the only caller of `P2pBridge`.

import 'dart:async';

import 'package:clock/clock.dart';
import 'package:flutter/foundation.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// How long `make_pairable` keeps this device open to pairing requests; matches Rust's window.
const Duration pairingWindow = Duration(minutes: 5);

/// How often the open Devices screen re-reads the status, so codes and requests appear quickly.
const Duration devicesPollInterval = Duration(seconds: 2);

/// Loads the P2P status and runs the owner's pairing actions.
final class DevicesController extends ChangeNotifier {
  /// Creates a controller over `p2pBridge`; nothing loads until `refresh` or `startPolling`.
  DevicesController({required P2pBridge p2pBridge}) : _bridge = p2pBridge;

  final P2pBridge _bridge;

  /// The last status read; kept when a later read fails.
  P2pStatus? status;

  /// A message safe to show, or `null`; never contains Rust's error text.
  String? errorMessage;

  /// Repeats `refresh` while the screen is shown.
  Timer? _pollTimer;

  /// When the current pairing window closes, or `null` when it is not open.
  DateTime? _discoverableUntil;

  /// Set by `dispose`, so late answers do not notify a dead controller.
  bool _disposed = false;

  /// Whether this device accepts pairing requests right now.
  bool get discoverable {
    final DateTime? until = _discoverableUntil;
    return until != null && clock.now().isBefore(until);
  }

  /// Reads the status.
  ///
  /// Outputs: Completes when the status or the error is stored.
  /// Side effects: Calls the bridge; clears `errorMessage` on success, sets it on failure while
  /// keeping the previous status; notifies listeners.
  Future<void> refresh() async {
    try {
      final P2pStatus latest = await _bridge.getStatus();
      status = latest;
      errorMessage = null;
    } on Object {
      errorMessage = l10n.unableToLoadDevices;
    }
    _notify();
  }

  /// Starts refreshing now and every `devicesPollInterval` until `stopPolling`.
  ///
  /// Side effects: Starts a periodic timer; replaces one already running.
  void startPolling() {
    _pollTimer?.cancel();
    unawaited(refresh());
    _pollTimer =
        Timer.periodic(devicesPollInterval, (_) => unawaited(refresh()));
  }

  /// Stops the timer started by `startPolling`.
  void stopPolling() {
    _pollTimer?.cancel();
    _pollTimer = null;
  }

  /// Opens this device to pairing requests for `pairingWindow`.
  ///
  /// Side effects: Sends `make_pairable`; on success records the window's end and refreshes.
  Future<void> makeDiscoverable() async {
    if (await _run('make_pairable')) {
      _discoverableUntil = clock.now().add(pairingWindow);
      await refresh();
    }
  }

  /// Asks the nearby device `peerId` to pair; both devices then show a code.
  Future<void> startPairing(String peerId) =>
      _runAndRefresh('start_pairing', peerId);

  /// Confirms that the code shown for the outgoing pairing with `peerId` matches.
  Future<void> confirmCode(String peerId) =>
      _runAndRefresh('confirm_code', peerId);

  /// Abandons the outgoing pairing with `peerId`.
  Future<void> cancelPairing(String peerId) =>
      _runAndRefresh('cancel_pairing', peerId);

  /// Accepts the incoming request from `peerId` after the owner compared codes.
  Future<void> acceptRequest(String peerId) =>
      _runAndRefresh('accept_request', peerId);

  /// Rejects the incoming request from `peerId`.
  Future<void> rejectRequest(String peerId) =>
      _runAndRefresh('reject_request', peerId);

  /// Removes the paired device `peerId`; it stops syncing with this device.
  Future<void> removeDevice(String peerId) => _runAndRefresh('revoke', peerId);

  /// Makes an invite to show on another device, and opens this device to pairing for as long.
  ///
  /// Outputs: The invite, or `null` with `errorMessage` set.
  /// Side effects: Opens the pairing window, records its end, and refreshes.
  Future<PairingInviteLink?> createInvite() async {
    try {
      final PairingInviteLink invite = await _bridge.createInvite();
      _discoverableUntil = invite.expiresAt;
      await refresh();
      return invite;
    } on P2pException catch (error) {
      errorMessage = error.errorCode == 'no_addresses'
          ? l10n.thisDeviceIsNotOnA
          : l10n.thatDidNotWorkTryAgain;
    } on Object {
      errorMessage = l10n.thatDidNotWorkTryAgain;
    }
    _notify();
    return null;
  }

  /// Reads an invite from another device, which then appears under Nearby devices.
  ///
  /// Inputs: Text as pasted; it is untrusted, and Rust checks it.
  /// Outputs: `null` on success, or a message for the owner. Nothing changes on failure.
  /// Side effects: Refreshes on success. No trust is granted: pairing still needs the code.
  Future<String?> joinInvite(String inviteText) async {
    try {
      await _bridge.joinInvite(inviteText);
    } on P2pException catch (error) {
      return _inviteRefusal(error.errorCode);
    } on Object {
      return l10n.thatDidNotWorkTryAgain;
    }
    await refresh();
    return null;
  }

  /// Finds the device an invite came from and starts pairing with it (#122): the invite link a
  /// phone's camera opened in the app.
  ///
  /// Inputs: The link's text; it is untrusted, and Rust checks it.
  /// Outputs: `null` once pairing started, or a message for the owner.
  /// Side effects: Dials the inviting device and refreshes. No trust is granted: both devices
  /// still show a code the owner compares before confirming.
  Future<String?> pairWithInvite(String inviteText) async {
    final String peerId;
    try {
      peerId = await _bridge.joinInvite(inviteText);
    } on P2pException catch (error) {
      return _inviteRefusal(error.errorCode);
    } on Object {
      return l10n.thatDidNotWorkTryAgain;
    }
    if (await _run('start_pairing', peerId: peerId)) {
      await refresh();
      return null;
    }
    return errorMessage ?? l10n.thatDidNotWorkTryAgain;
  }

  /// Renames this device (#121); nearby and connected paired devices see the new name at once,
  /// other paired devices when they next connect.
  ///
  /// Inputs: The name as typed; Rust trims and checks it.
  /// Outputs: `null` once saved, or a message for the owner; nothing changes on failure.
  /// Side effects: Writes the saved name through the bridge and refreshes.
  Future<String?> renameDevice(String name) async {
    try {
      await _bridge.setDeviceName(name);
    } on P2pException catch (error) {
      return switch (error.errorCode) {
        'invalid_device_name' => l10n.use1To64CharactersWithout,
        'storage_failed' => l10n.theNameCouldNotBeSaved,
        _ => l10n.thatDidNotWorkTryAgain,
      };
    } on Object {
      return l10n.thatDidNotWorkTryAgain;
    }
    await refresh();
    return null;
  }

  /// Words an invite's refusal for the owner.
  String _inviteRefusal(String errorCode) => switch (errorCode) {
        'invite_expired' => l10n.thisInviteHasExpiredMakeA,
        'invite_from_this_device' => l10n.thisInviteWasMadeOnThis,
        'already_paired' => l10n.thatDeviceIsAlreadyPaired,
        'invite_malformed' => l10n.thatIsNotAValidInvite,
        'unavailable' => l10n.deviceSyncIsNotRunning,
        _ => l10n.thatDidNotWorkTryAgain,
      };

  /// Runs a peer command and refreshes when it succeeds.
  Future<void> _runAndRefresh(String command, String peerId) async {
    if (await _run(command, peerId: peerId)) {
      await refresh();
    }
  }

  /// Runs `command`, storing a readable message when it fails.
  ///
  /// Outputs: Whether the command succeeded.
  /// Side effects: Calls the bridge; on failure sets `errorMessage`, keeps the status, and
  /// notifies listeners.
  Future<bool> _run(String command, {String? peerId}) async {
    try {
      await _bridge.runCommand(command, peerId: peerId);
      return true;
    } on P2pException catch (error) {
      errorMessage = switch (error.errorCode) {
        'refused' => l10n.theOtherDeviceIsNoLonger,
        'unavailable' => l10n.deviceSyncIsNotRunning,
        _ => l10n.thatDidNotWorkTryAgain,
      };
    } on Object {
      errorMessage = l10n.thatDidNotWorkTryAgain;
    }
    _notify();
    return false;
  }

  /// Notifies listeners unless disposed.
  void _notify() {
    if (!_disposed) {
      notifyListeners();
    }
  }

  @override
  void dispose() {
    _disposed = true;
    stopPolling();
    super.dispose();
  }
}
