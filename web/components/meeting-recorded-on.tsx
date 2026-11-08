// Names the devices a meeting was recorded on when the app combined several devices' recordings
// of it (#88), says how far combining them got, and splits them apart again after asking.

"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { splitMeeting, type RecordedOn } from "@/lib/api";
import { describeCombining, describeRecordedOn } from "@/lib/meeting-metadata";

/**
 * "Recorded on Laptop + Pixel" with a Split button, then how far combining got; nothing for a
 * meeting from one device of an older daemon.
 *
 * Inputs: the meeting's ID, `recorded_on`, `combined_recordings` and `combining`. Outputs: the
 * lines, an inline confirmation, and a polite status line. Side effects: `POST /api/meetings/{id}/split` once
 * confirmed, then refreshes the page so the list and transcript match the daemon.
 */
export function MeetingRecordedOn({ meeting }: { meeting: RecordedOn & { meeting_id: string } }) {
  const router = useRouter();
  const [isConfirming, setIsConfirming] = useState(false);
  const [isSplitting, setIsSplitting] = useState(false);
  const [message, setMessage] = useState("");
  const description = describeRecordedOn(meeting);
  const combining = describeCombining(meeting.combining);
  if (description === null && combining === null && message === "") return null;

  /** Splits the meeting and reports the outcome. */
  async function split() {
    setIsSplitting(true);
    try {
      const listedAgain = await splitMeeting(meeting.meeting_id);
      setMessage(
        listedAgain.length === 0
          ? "Nothing to split"
          : `Split: ${listedAgain.join(", ")} ${listedAgain.length === 1 ? "is" : "are"} listed on ${listedAgain.length === 1 ? "its" : "their"} own again`
      );
      router.refresh();
    } catch {
      setMessage("Unable to split this meeting");
    } finally {
      setIsSplitting(false);
      setIsConfirming(false);
    }
  }

  return (
    <div className="my-2">
      {description === null ? null : (
        <p className="flex flex-wrap items-center gap-2 text-muted">
          <span>{description}</span>
          {isConfirming ? (
            <span className="inline-flex flex-wrap items-center gap-2">
              <span>The other recordings are listed on their own again and never joined again.</span>
              <button
                type="button"
                disabled={isSplitting}
                onClick={split}
                className="min-h-11 rounded-lg bg-accent px-3 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled focus-visible:outline-2 focus-visible:outline-focus"
              >
                Split for good
              </button>
              <button
                type="button"
                onClick={() => setIsConfirming(false)}
                className="min-h-11 rounded-lg px-3 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus"
              >
                Cancel
              </button>
            </span>
          ) : (
            <button
              type="button"
              onClick={() => {
                setMessage("");
                setIsConfirming(true);
              }}
              className="min-h-11 rounded-lg border border-line-strong px-3 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus"
            >
              Split
            </button>
          )}
        </p>
      )}
      {combining === null ? null : <p className="text-sm text-muted">{combining}</p>}
      <p role="status" aria-live="polite" className="text-sm">
        {message}
      </p>
    </div>
  );
}
