// Names the devices a meeting was recorded on when the app combined several devices' recordings
// of it (#88), says how far combining them got, and splits them apart again after asking.

"use client";

import { useRouter } from "next/navigation";
import { useState } from "react";
import { splitMeeting, type RecordedOn } from "@/lib/api";
import { describeCombining, describeRecordedOn } from "@/lib/meeting-metadata";
import { useT } from "@/lib/i18n/client";

/**
 * "Recorded on Laptop + Pixel" with a Split button, then how far combining got; nothing for a
 * meeting from one device of an older daemon.
 *
 * Inputs: the meeting's ID, `recorded_on`, `combined_recordings` and `combining`. Outputs: the
 * lines, an inline confirmation, and a polite status line. Side effects: `POST /api/meetings/{id}/split` once
 * confirmed, then refreshes the page so the list and transcript match the daemon.
 */
export function MeetingRecordedOn({ meeting }: { meeting: RecordedOn & { meeting_id: string } }) {
  const t = useT();
  const router = useRouter();
  const [isConfirming, setIsConfirming] = useState(false);
  const [isSplitting, setIsSplitting] = useState(false);
  const [message, setMessage] = useState("");
  const description = describeRecordedOn(meeting, t);
  const combining = describeCombining(meeting.combining, t);
  if (description === null && combining === null && message === "") return null;

  /** Splits the meeting and reports the outcome. */
  async function split() {
    setIsSplitting(true);
    try {
      const listedAgain = await splitMeeting(meeting.meeting_id);
      setMessage(
        listedAgain.length === 0
          ? t("nothingToSplit")
          : t("splitListedAgain", { recordings: listedAgain.join(", "), count: listedAgain.length })
      );
      router.refresh();
    } catch {
      setMessage(t("unableToSplitThisMeeting"));
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
              <span>{t("theOtherRecordingsAreListed")}</span>
              <button
                type="button"
                disabled={isSplitting}
                onClick={split}
                className="min-h-11 rounded-lg bg-accent px-3 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled focus-visible:outline-2 focus-visible:outline-focus"
              >{t("splitForGood")}</button>
              <button
                type="button"
                onClick={() => setIsConfirming(false)}
                className="min-h-11 rounded-lg px-3 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus"
              >{t("cancel")}</button>
            </span>
          ) : (
            <button
              type="button"
              onClick={() => {
                setMessage("");
                setIsConfirming(true);
              }}
              className="min-h-11 rounded-lg border border-line-strong px-3 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus"
            >{t("split")}</button>
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
