// Plain messages for deletions that fail; server text is not shown.

import { DeletionError, deleteMeeting } from "@/lib/api";
import { englishT, type Translate } from "@/lib/i18n";

/** What goes when a meeting is deleted, for the confirmation. */
export const meetingDeletionMessage = (t: Translate) => t("itsTranscriptAttachmentsAndUnknown");

/**
 * Deletes a meeting, turning a failure into a message the dialog can show.
 *
 * Inputs: the meeting. Outputs: resolves when it is gone; throws an `Error` with plain text.
 * Side effects: as `deleteMeeting`.
 */
export async function deleteMeetingWithMessage(meetingId: string, t: Translate = englishT): Promise<void> {
  try {
    await deleteMeeting(meetingId);
  } catch (error) {
    throw new Error(
      error instanceof DeletionError && error.kind === "recording"
        ? t("thisMeetingIsBeingRecorded")
        : t("couldntDeleteThisMeetingCheck")
    );
  }
}
