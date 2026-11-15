// Renders the dialog that renames this device (Issue #121): the name nearby devices list while
// this one is open to pairing, and the one devices learn when they pair with it.

import 'dart:math' as math;

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/devices_controller.dart';

/// Preferred width of the name field, in logical pixels.
const double _preferredFieldWidth = 400;

/// Horizontal space an `AlertDialog` takes around its content: 40 inset and 24 padding per side.
const double _dialogHorizontalChrome = 128;

/// Longest name Rust accepts, in Unicode characters (`DeviceName::MAX_CHARACTERS`).
const int maxDeviceNameCharacters = 64;

/// Opens the rename dialog over the Devices screen.
///
/// Inputs: A context below a `Navigator`, the controller, and the current name.
/// Outputs: Completes when the dialog closes.
/// Side effects: Saves the name through `controller` when the owner presses Save.
Future<void> showRenameDeviceDialog(
  BuildContext context, {
  required DevicesController controller,
  required String currentName,
}) =>
    showDialog<void>(
      context: context,
      builder: (BuildContext context) =>
          RenameDeviceDialog(controller: controller, currentName: currentName),
    );

/// Lets the owner edit this device's name; Rust checks it, and a refusal stays in the dialog.
final class RenameDeviceDialog extends StatefulWidget {
  /// Creates a dialog that starts from `currentName`.
  const RenameDeviceDialog({
    required this.controller,
    required this.currentName,
    super.key,
  });

  /// Saves the name.
  final DevicesController controller;

  /// The name shown now; the field starts from it, selected.
  final String currentName;

  @override
  State<RenameDeviceDialog> createState() => _RenameDeviceDialogState();
}

/// Holds the field text, the save state, and the latest refusal.
final class _RenameDeviceDialogState extends State<RenameDeviceDialog> {
  late final TextEditingController _nameController;

  /// Whether a save is in flight; Save, Cancel, and Escape are disabled meanwhile.
  bool _isSaving = false;

  /// Why the latest save was refused, or `null`.
  String? _refusal;

  @override
  void initState() {
    super.initState();
    _nameController = TextEditingController.fromValue(TextEditingValue(
      text: widget.currentName,
      selection:
          TextSelection(baseOffset: 0, extentOffset: widget.currentName.length),
    ));
  }

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  /// Saves the typed name; closes on success, otherwise shows the refusal under the field.
  ///
  /// Side effects: Calls Rust through the controller.
  Future<void> _save() async {
    if (_isSaving) {
      return;
    }
    setState(() {
      _isSaving = true;
      _refusal = null;
    });
    final String? refusal =
        await widget.controller.renameDevice(_nameController.text);
    if (!mounted) {
      return;
    }
    if (refusal == null) {
      Navigator.of(context).pop();
      return;
    }
    setState(() {
      _isSaving = false;
      _refusal = refusal;
    });
  }

  @override
  Widget build(BuildContext context) {
    final double fieldWidth = math.max(
        0,
        math.min(_preferredFieldWidth,
            MediaQuery.sizeOf(context).width - _dialogHorizontalChrome));
    return PopScope(
      canPop: !_isSaving,
      child: AlertDialog(
        title: const Text('Rename this device'),
        content: SizedBox(
          width: fieldWidth,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: <Widget>[
              TextField(
                controller: _nameController,
                autofocus: true,
                enabled: !_isSaving,
                textInputAction: TextInputAction.done,
                textCapitalization: TextCapitalization.words,
                onChanged: (_) => setState(() => _refusal = null),
                onSubmitted: (_) => _save(),
                decoration: InputDecoration(
                  labelText: 'Name',
                  border: const OutlineInputBorder(),
                  errorText: _refusal,
                  errorMaxLines: 3,
                  // Counted in Unicode characters, as Rust counts them.
                  counterText:
                      '${_nameController.text.trim().runes.length}/$maxDeviceNameCharacters',
                ),
              ),
              const SizedBox(height: 8),
              Text(
                'Nearby devices see this name while this device is discoverable. Devices '
                'already paired keep the name they learned.',
                style: Theme.of(context).textTheme.bodySmall,
              ),
            ],
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: _isSaving ? null : () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: _isSaving ? null : _save,
            child: _isSaving
                ? const SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : const Text('Save'),
          ),
        ],
      ),
    );
  }
}
