// Renders the web Settings switch for keeping online-meeting participants' voices in the voice
// gallery (#103). Voice prints are biometric, so it is off until the owner turns it on, and a
// meeting room or other track with several voices is never kept.

"use client";

import { useId, useState } from "react";
import {
  MeetingVoicesError,
  setMeetingVoices,
  type MeetingVoices,
  type MeetingVoicesFailureKind
} from "@/lib/meeting-voices-api";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";

/** Plain messages per failure; server text is never shown. */
const failureMessages = (t: Translate) : Record<MeetingVoicesFailureKind, string> => ({
  unavailable: t("thisServerCantKeepMeeting"),
  server: t("somethingWentWrongOnThe"),
  network: t("cantReachTheNoteTaking")
});

export type MeetingVoicesSettingsProps = {
  /** The saved choice, or `null` when it could not be read. */
  initial: MeetingVoices | null;
};

/**
 * Lets the owner choose whether online-meeting participants' voices are kept.
 *
 * Inputs: see `MeetingVoicesSettingsProps`. Outputs: a labelled switch, its explanation and a
 * status line announced to screen readers. Side effects: `PUT /api/settings/meeting-voices` when
 * switched; a failed save goes back to the saved choice.
 */
export function MeetingVoicesSettings({ initial }: MeetingVoicesSettingsProps) {
  const t = useT();
  const [keep, setKeep] = useState(initial?.keep ?? false);
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? t("cantReadWhetherMeetingVoices") : null
  );
  const hintId = useId();

  /** Saves the choice and shows it as the server kept it. */
  async function choose(next: boolean) {
    setIsSaving(true);
    setMessage(null);
    try {
      const saved = await setMeetingVoices(next);
      setKeep(saved.keep);
      setMessage(
        saved.keep
          ? t("savedParticipantsVoicesWillBe")
          : t("savedParticipantsVoicesWillNo")
      );
    } catch (error) {
      setMessage(failureMessages(t)[error instanceof MeetingVoicesError ? error.kind : "server"]);
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="card max-w-2xl space-y-3">
      <label className="flex min-h-11 items-center gap-2 font-semibold">
        <input
          type="checkbox"
          role="switch"
          aria-describedby={hintId}
          checked={keep}
          disabled={initial === null || isSaving}
          onChange={(event) => void choose(event.target.checked)}
        />{t("keepMeetingParticipantsVoices")}</label>
      <p id={hintId} className="text-sm text-muted">{t("inOnlineMeetingsAParticipant")}</p>
      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}
