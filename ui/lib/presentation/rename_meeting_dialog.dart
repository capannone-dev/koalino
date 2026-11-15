// Renders the Material 3 dialog that renames a meeting, shared by the list and the detail.

import 'dart:math' as math;

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/controllers/meeting_title_validation.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Preferred width of the title field, in logical pixels.
const double _preferredFieldWidth = 400;

/// Horizontal space an `AlertDialog` takes around its content: 40 inset and 24 padding per side.
const double _dialogHorizontalChrome = 128;

/// Opens the rename dialog for `meeting` and confirms a successful rename with a snack bar.
///
/// Inputs: A context below a `Navigator` and a `ScaffoldMessenger`, the shared controller, and
/// the meeting's current summary.
/// Outputs: Completes when the dialog closes.
/// Side effects: Renames the meeting through `controller` when the person saves a new title, and
/// shows "Meeting renamed". An unchanged title closes the dialog without calling Rust.
Future<void> showRenameMeetingDialog(
  BuildContext context, {
  required MeetingListController controller,
  required MeetingSummary meeting,
}) async {
  final ScaffoldMessengerState? messenger = ScaffoldMessenger.maybeOf(context);
  final bool? renamed = await showDialog<bool>(
    context: context,
    builder: (BuildContext context) =>
        RenameMeetingDialog(controller: controller, meeting: meeting),
  );
  if (renamed ?? false) {
    messenger?.showSnackBar(SnackBar(content: Text(l10n.meetingRenamed)));
  }
}

/// Lets the person edit a meeting title, validates it like Rust, and saves it.
///
/// Pops `true` after a rename, `false` when the title was unchanged, and `null` on Cancel or
/// Escape.
final class RenameMeetingDialog extends StatefulWidget {
  /// Creates a dialog renaming `meeting` through `controller`.
  const RenameMeetingDialog({
    required this.controller,
    required this.meeting,
    super.key,
  });

  final MeetingListController controller;
  final MeetingSummary meeting;

  @override
  State<RenameMeetingDialog> createState() => _RenameMeetingDialogState();
}

/// Holds the field text, the save state, and the latest error.
final class _RenameMeetingDialogState extends State<RenameMeetingDialog> {
  late final TextEditingController _titleController;

  /// Whether a rename is in flight; Save, Cancel, and Escape are disabled meanwhile.
  bool _isSaving = false;

  /// Whether the person tried to save, after which validation errors update as they type.
  bool _hasAttemptedSave = false;

  /// Why Rust rejected the latest save, or `null`.
  String? _saveErrorMessage;

  @override
  void initState() {
    super.initState();
    final String currentTitle = widget.meeting.title;
    _titleController = TextEditingController.fromValue(TextEditingValue(
      text: currentTitle,
      selection:
          TextSelection(baseOffset: 0, extentOffset: currentTitle.length),
    ));
  }

  @override
  void dispose() {
    _titleController.dispose();
    super.dispose();
  }

  /// Validates the field and, when it holds a new valid title, renames the meeting.
  ///
  /// Side effects: Calls Rust through the controller, then pops the dialog on success or shows
  /// the failure under the field.
  Future<void> _save() async {
    if (_isSaving) {
      return;
    }
    final MeetingTitleValidation validation =
        validateMeetingTitle(_titleController.text);
    setState(() {
      _hasAttemptedSave = true;
      _saveErrorMessage = null;
    });
    if (!validation.isValid) {
      return;
    }
    if (validation.trimmedTitle == widget.meeting.title) {
      Navigator.of(context).pop(false);
      return;
    }
    setState(() => _isSaving = true);
    String? saveErrorMessage;
    try {
      await widget.controller
          .renameMeeting(widget.meeting.meetingId, validation.trimmedTitle);
    } on MeetingRenameException catch (error) {
      saveErrorMessage = _renameFailureMessage(error.failure);
    } on Object {
      saveErrorMessage = _renameFailureMessage(MeetingRenameFailure.unknown);
    }
    if (!mounted) {
      return;
    }
    if (saveErrorMessage == null) {
      Navigator.of(context).pop(true);
      return;
    }
    setState(() {
      _isSaving = false;
      _saveErrorMessage = saveErrorMessage;
    });
  }

  @override
  Widget build(BuildContext context) {
    final String typedTitle = _titleController.text;
    final String? validationMessage = _hasAttemptedSave
        ? validateMeetingTitle(typedTitle).errorMessage
        : null;
    final double fieldWidth = math.max(
        0,
        math.min(_preferredFieldWidth,
            MediaQuery.sizeOf(context).width - _dialogHorizontalChrome));
    return PopScope(
      canPop: !_isSaving,
      child: AlertDialog(
        title: Text(l10n.renameMeeting),
        content: SizedBox(
          width: fieldWidth,
          child: TextField(
            controller: _titleController,
            autofocus: true,
            enabled: !_isSaving,
            textInputAction: TextInputAction.done,
            textCapitalization: TextCapitalization.sentences,
            onChanged: (_) => setState(() => _saveErrorMessage = null),
            onSubmitted: (_) => _save(),
            decoration: InputDecoration(
              labelText: l10n.title,
              border: const OutlineInputBorder(),
              errorText: validationMessage ?? _saveErrorMessage,
              errorMaxLines: 3,
              // Counted in Unicode characters, as Rust counts them.
              counterText:
                  '${typedTitle.trim().runes.length}/$maxMeetingTitleCharacters',
            ),
          ),
        ),
        actions: <Widget>[
          TextButton(
            onPressed: _isSaving ? null : () => Navigator.of(context).pop(),
            child: Text(l10n.cancel),
          ),
          FilledButton(
            onPressed: _isSaving ? null : _save,
            child: _isSaving
                ? const SizedBox.square(
                    dimension: 18,
                    child: CircularProgressIndicator(strokeWidth: 2),
                  )
                : Text(l10n.save),
          ),
        ],
      ),
    );
  }
}

/// Describes why Rust rejected a rename, without exposing Rust's internal error details.
String _renameFailureMessage(MeetingRenameFailure failure) => switch (failure) {
      MeetingRenameFailure.invalidTitle => l10n.thisTitleIsnTAllowed,
      MeetingRenameFailure.storageFailure => l10n.couldnTSaveTheNewTitle,
      MeetingRenameFailure.runtimeUnavailable => l10n.theNoteTakingEngineIsnT,
      MeetingRenameFailure.meetingNotFound => l10n.thisMeetingNoLongerExists2,
      MeetingRenameFailure.unknown => l10n.couldnTRenameTheMeeting,
    };
