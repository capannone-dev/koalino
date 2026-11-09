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

/** Plain messages per failure; server text is never shown. */
const failureMessages: Record<MeetingVoicesFailureKind, string> = {
  unavailable: "This server can't keep meeting voices.",
  server: "Something went wrong on the server. Try again.",
  network: "Can't reach the note-taking server. Check that it is running."
};

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
  const [keep, setKeep] = useState(initial?.keep ?? false);
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? "Can't read whether meeting voices are kept. Check that the note-taking server is running." : null
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
          ? "Saved. Participants' voices will be kept after enough speech of one voice."
          : "Saved. Participants' voices will no longer be kept; voices kept earlier stay in the gallery."
      );
    } catch (error) {
      setMessage(failureMessages[error instanceof MeetingVoicesError ? error.kind : "server"]);
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
        />
        Keep meeting participants&apos; voices
      </label>
      <p id={hintId} className="text-sm text-muted">
        In online meetings, a participant who spoke alone for about 30 seconds gets their voice in the voice gallery,
        so they are recognised later, also in the room. A meeting room or a shared computer, with several voices, is
        never kept. Voice prints are biometric data: tell participants, and remove a voice in the gallery when asked.
      </p>
      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}
