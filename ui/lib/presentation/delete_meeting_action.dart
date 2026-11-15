// Asks before deleting a meeting, deletes it, and reports a refusal. Shared by the list and detail.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/confirm_delete_dialog.dart';
import 'package:note_taker_ui/l10n/l10n.dart';

/// Deletes `meeting` once the owner confirms.
///
/// Inputs: A mounted context, the controller, and the meeting.
/// Outputs: `true` when the meeting was deleted.
/// Side effects: Shows a dialog, deletes through the controller, and shows a snack bar on refusal.
Future<bool> deleteMeetingAfterAsking(
  BuildContext context, {
  required MeetingListController controller,
  required MeetingSummary meeting,
}) async {
  final ScaffoldMessengerState? messenger = ScaffoldMessenger.maybeOf(context);
  final bool confirmed = await confirmDelete(
    context,
    title: l10n.delete2(meeting.title),
    message: l10n.itsTranscriptAttachmentsAndUnknownVoices,
  );
  if (!confirmed) {
    return false;
  }
  final bool deleted = await controller.deleteMeeting(meeting.meetingId);
  if (!deleted) {
    messenger?.showSnackBar(SnackBar(
      content: Text(l10n.unableToDeleteThisMeetingIf),
    ));
  }
  return deleted;
}
