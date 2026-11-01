// Plain messages for deletions that fail; server text is not shown.

import { DeletionError, deleteMeeting } from "@/lib/api";

/** What goes when a meeting is deleted, for the confirmation. */
export const meetingDeletionMessage =
  "Its transcript, attachments and unknown voices go for good, here and on your paired devices. People you saved stay.";

/**
 * Deletes a meeting, turning a failure into a message the dialog can show.
 *
 * Inputs: the meeting. Outputs: resolves when it is gone; throws an `Error` with plain text.
 * Side effects: as `deleteMeeting`.
 */
export async function deleteMeetingWithMessage(meetingId: string): Promise<void> {
  try {
    await deleteMeeting(meetingId);
  } catch (error) {
    throw new Error(
      error instanceof DeletionError && error.kind === "recording"
        ? "This meeting is being recorded. Stop the recording first."
        : "Couldn't delete this meeting. Check that the note-taking server is running."
    );
  }
}
