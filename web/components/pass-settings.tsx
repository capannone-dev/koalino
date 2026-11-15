// Renders the web Settings page's after-meeting pass (#87): transcribe each meeting again after
// Stop, with more context, and choose the model for it.

"use client";

import { useId, useState } from "react";
import { PassError, setAfterMeetingPass, type AfterMeetingPass, type PassFailureKind } from "@/lib/pass-api";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";

/** Plain messages per failure; server text is never shown because it can contain internal details. */
const failureMessages = (t: Translate) : Record<PassFailureKind, string> => ({
  invalid_model: t("thatModelIsNotOn"),
  unavailable: t("thisServerCantChangeThe"),
  server: t("somethingWentWrongOnThe"),
  network: t("cantReachTheNoteTaking")
});

/** The menu value standing for the recording model. */
const sameAsRecording = "";

export type PassSettingsProps = {
  /** The saved choices, or `null` when they could not be read. */
  initial: AfterMeetingPass | null;
};

/**
 * Lets the owner turn the after-meeting pass on or off and choose its model.
 *
 * Inputs: see `PassSettingsProps`. Outputs: a labelled switch, a model menu while the pass is on,
 * and a status line announced to screen readers. Side effects: `PUT
 * /api/settings/after-meeting-pass` on each change.
 */
export function PassSettings({ initial }: PassSettingsProps) {
  const t = useT();
  const [pass, setPass] = useState<AfterMeetingPass>(initial ?? { enabled: true, model: null, models: [] });
  const [isSaving, setIsSaving] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? t("cantReadTheAfterMeeting") : null
  );
  const switchId = useId();
  const menuId = useId();
  const hintId = useId();

  /** Saves one change and shows the choices as the server kept them. */
  async function save(change: { enabled?: boolean; model?: string | null }) {
    setIsSaving(true);
    setMessage(null);
    try {
      setPass(await setAfterMeetingPass(change));
      setMessage(t("savedMeetingsRecordedFromNow"));
    } catch (error) {
      setMessage(failureMessages(t)[error instanceof PassError ? error.kind : "server"]);
    } finally {
      setIsSaving(false);
    }
  }

  const disabled = initial === null || isSaving;
  return (
    <div className="card max-w-2xl space-y-3">
      <div className="flex items-start gap-3">
        <input
          id={switchId}
          type="checkbox"
          className="mt-1 h-5 w-5"
          aria-describedby={hintId}
          checked={pass.enabled}
          disabled={disabled}
          onChange={(event) => void save({ enabled: event.target.checked })}
        />
        <div className="space-y-1">
          <label htmlFor={switchId} className="block font-semibold">{t("improveTheTranscriptAfterThe")}</label>
          <p id={hintId} className="text-sm text-muted">{t("transcribesEachMeetingAgainWith")}</p>
        </div>
      </div>
      {pass.enabled ? (
        <div className="space-y-1">
          <label htmlFor={menuId} className="block font-semibold">{t("modelForThisPass")}</label>
          <select
            id={menuId}
            className="min-h-11 rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 disabled:text-faint"
            value={pass.model ?? sameAsRecording}
            disabled={disabled}
            onChange={(event) => void save({ model: event.target.value === sameAsRecording ? null : event.target.value })}
          >
            <option value={sameAsRecording}>{t("sameAsForRecording")}</option>
            {pass.models.map((model) => (
              <option key={model.file_name} value={model.file_name}>
                {model.name}
              </option>
            ))}
          </select>
        </div>
      ) : null}
      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}
