// Asks before splitting a meeting the app combined from several devices' recordings (#88), splits
// it, and reports a refusal. Used by the meeting detail.

// ignore: deprecated_member_use
import 'package:flutter/material.dart';
import 'package:note_taker_ui/controllers/meeting_list_controller.dart';
import 'package:note_taker_ui/infrastructure/meeting_bridge.dart';

/// Splits `meeting` into its devices' recordings once the owner confirms.
///
/// Inputs: A mounted context, the controller, and a meeting with combined recordings.
/// Outputs: `true` when the recordings were split.
/// Side effects: Shows a dialog, splits through the controller (which reloads the list and the
/// detail), and shows a snack bar on refusal.
Future<bool> splitMeetingAfterAsking(
  BuildContext context, {
  required MeetingListController controller,
  required MeetingSummary meeting,
}) async {
  final ScaffoldMessengerState? messenger = ScaffoldMessenger.maybeOf(context);
  final int otherCount = meeting.combinedRecordings.length;
  final bool? confirmed = await showDialog<bool>(
    context: context,
    builder: (BuildContext dialogContext) => AlertDialog(
      title: const Text('Split this meeting?'),
      content: Text(
        'It was recorded on ${meeting.recordedOnLabel ?? 'several devices'}, '
        'and the app joined the recordings. Splitting lists the other '
        '${otherCount == 1 ? 'recording' : '$otherCount recordings'} on its own '
        'again, with its own transcript, here and on your paired devices. '
        'They are not joined again. This meeting keeps its transcript.',
      ),
      actions: <Widget>[
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(false),
          child: const Text('Cancel'),
        ),
        FilledButton(
          onPressed: () => Navigator.of(dialogContext).pop(true),
          child: const Text('Split'),
        ),
      ],
    ),
  );
  if (!(confirmed ?? false)) {
    return false;
  }
  final bool split = await controller.splitMeeting(meeting.meetingId);
  if (!split) {
    messenger?.showSnackBar(const SnackBar(
      content: Text('Unable to split this meeting.'),
    ));
  }
  return split;
}
