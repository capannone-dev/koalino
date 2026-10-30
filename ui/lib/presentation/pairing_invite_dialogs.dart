// Shows a pairing invite as a QR code and a copyable link, and takes one pasted from another device
// (Issue #65). An invite only helps the devices find each other: they still compare a code.

import 'package:flutter/services.dart';
// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';
import 'package:note_taker_ui/infrastructure/p2p_bridge.dart';
import 'package:qr_flutter/qr_flutter.dart';

/// Makes an invite and shows it; a failure appears in the Devices screen's error line.
///
/// Inputs: The context to show in, and the controller that makes the invite.
/// Side effects: Opens this device to pairing until the invite ends, and shows a dialog.
Future<void> showInviteDialog(
    BuildContext context, DevicesController controller) async {
  final PairingInviteLink? invite = await controller.createInvite();
  if (invite == null || !context.mounted) {
    return;
  }
  await showDialog<void>(
    context: context,
    builder: (BuildContext dialogContext) => _InviteDialog(invite: invite),
  );
}

/// Asks for invite text and passes it to the controller.
///
/// Side effects: Shows a dialog; on success the invite's device is listed under Nearby devices.
Future<void> showJoinInviteDialog(
        BuildContext context, DevicesController controller) =>
    showDialog<void>(
      context: context,
      builder: (BuildContext dialogContext) =>
          _JoinInviteDialog(controller: controller),
    );

/// The invite as a QR code and as text with a Copy button.
final class _InviteDialog extends StatelessWidget {
  const _InviteDialog({required this.invite});

  final PairingInviteLink invite;

  @override
  Widget build(BuildContext context) {
    final String minute = invite.expiresAt.minute.toString().padLeft(2, '0');
    return AlertDialog(
      title: const Text('Invite a device'),
      content: SizedBox(
        width: 320,
        child: SingleChildScrollView(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: <Widget>[
              // A light background in both themes, since scanners expect dark on light.
              ColoredBox(
                color: Colors.white,
                child: QrImageView(
                  data: invite.text,
                  size: 240,
                  semanticsLabel: 'Pairing invite QR code',
                ),
              ),
              const SizedBox(height: 12),
              Text(
                'On the other device, open Devices, choose Use invite, and paste this link. '
                'You can scan the code with a camera app to copy it. '
                'It works until ${invite.expiresAt.hour}:$minute.',
              ),
              const SizedBox(height: 12),
              SelectableText(invite.text,
                  style: Theme.of(context).textTheme.bodySmall),
            ],
          ),
        ),
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(context).pop(),
          child: const Text('Close'),
        ),
        FilledButton.icon(
          onPressed: () async {
            await Clipboard.setData(ClipboardData(text: invite.text));
            if (context.mounted) {
              ScaffoldMessenger.of(context).showSnackBar(
                  const SnackBar(content: Text('Invite link copied')));
            }
          },
          icon: const Icon(Icons.copy),
          label: const Text('Copy link'),
        ),
      ],
    );
  }
}

/// A field for pasted invite text, with the reason shown when it is refused.
final class _JoinInviteDialog extends StatefulWidget {
  const _JoinInviteDialog({required this.controller});

  final DevicesController controller;

  @override
  State<_JoinInviteDialog> createState() => _JoinInviteDialogState();
}

/// Holds the typed text and the refusal message.
final class _JoinInviteDialogState extends State<_JoinInviteDialog> {
  final TextEditingController _inviteField = TextEditingController();
  String? _errorText;

  @override
  void dispose() {
    _inviteField.dispose();
    super.dispose();
  }

  /// Sends the text; closes on success and shows the reason otherwise.
  Future<void> _submit() async {
    final String? refusal =
        await widget.controller.joinInvite(_inviteField.text);
    if (!mounted) {
      return;
    }
    if (refusal == null) {
      Navigator.of(context).pop();
      ScaffoldMessenger.of(context).showSnackBar(const SnackBar(
          content: Text('Device found. Choose Pair under Nearby devices.')));
    } else {
      setState(() => _errorText = refusal);
    }
  }

  @override
  Widget build(BuildContext context) => AlertDialog(
        title: const Text('Use an invite'),
        content: TextField(
          controller: _inviteField,
          autofocus: true,
          minLines: 2,
          maxLines: 5,
          decoration: InputDecoration(
            labelText: 'Invite link',
            border: const OutlineInputBorder(),
            errorText: _errorText,
            errorMaxLines: 3,
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(onPressed: _submit, child: const Text('Find device')),
        ],
      );
}
