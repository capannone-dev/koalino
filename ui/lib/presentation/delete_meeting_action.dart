// Asks before deleting a meeting, deletes it, and reports a refusal. Shared by the list and detail.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';
import 'package:note_taker_ui/presentation/confirm_delete_dialog.dart';

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
    title: 'Delete ${meeting.title}?',
    message: 'Its transcript, attachments and unknown voices go for good, '
        'here and on your paired devices. People you saved stay.',
  );
  if (!confirmed) {
    return false;
  }
  final bool deleted = await controller.deleteMeeting(meeting.meetingId);
  if (!deleted) {
    messenger?.showSnackBar(const SnackBar(
      content: Text('Unable to delete this meeting. If it is being recorded, '
          'stop the recording first.'),
    ));
  }
  return deleted;
}
