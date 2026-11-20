// Renders the web Settings page's kept meeting recordings (#140): the switch, turned on only after
// a privacy and space warning, how many recordings are kept, and Wipe all recordings.

"use client";

import { useId, useState } from "react";
import {
  recordingsSize,
  setKeepRecordings,
  wipeRecordings,
  getRecordingsStatus,
  type RecordingsStatus
} from "@/lib/recordings-api";
import { useT } from "@/lib/i18n/client";

const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-4 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";

export type RecordingsSettingsProps = {
  /** The saved status, or `null` when it could not be read. */
  initial: RecordingsStatus | null;
};

/**
 * Lets the owner keep meeting recordings, and wipe them.
 *
 * Inputs: see `RecordingsSettingsProps`. Outputs: the switch with its warning, the count and size,
 * a Wipe all button with a confirmation, and a status line. Side effects: the `/api/settings/
 * recordings` and `/api/recordings` routes.
 */
export function RecordingsSettings({ initial }: RecordingsSettingsProps) {
  const t = useT();
  const [status, setStatus] = useState<RecordingsStatus | null>(initial);
  const [warning, setWarning] = useState(false);
  const [confirmWipe, setConfirmWipe] = useState(false);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(initial === null ? t("recordingsUnavailable") : null);
  const hintId = useId();
  const warningId = useId();

  /** Runs one change, showing a plain message when it fails. */
  async function run(work: () => Promise<string | null>) {
    setBusy(true);
    setMessage(null);
    try {
      setMessage(await work());
    } catch {
      // Server text is never shown: it can hold internal details.
      setMessage(t("recordingsUnavailable"));
    } finally {
      setBusy(false);
    }
  }

  function keep(on: boolean) {
    setWarning(false);
    void run(async () => {
      setStatus(await setKeepRecordings(on));
      return null;
    });
  }

  function wipe() {
    setConfirmWipe(false);
    void run(async () => {
      const deleted = await wipeRecordings();
      setStatus(await getRecordingsStatus());
      return t("recordingsWiped", { count: deleted });
    });
  }

  return (
    <div className="card max-w-2xl space-y-3">
      <label className="flex items-start gap-3">
        <input
          type="checkbox"
          className="mt-1 h-5 w-5"
          aria-describedby={hintId}
          checked={status?.keep ?? false}
          disabled={status === null || busy}
          onChange={(event) => (event.target.checked ? setWarning(true) : keep(false))}
        />
        <span>
          <span className="block font-semibold">{t("keepMeetingRecordings")}</span>
          <span id={hintId} className="block text-sm text-muted">{t("keepMeetingRecordingsSubtitle")}</span>
        </span>
      </label>
      {warning ? (
        <div role="alertdialog" aria-labelledby={warningId} className="space-y-2 rounded-lg border border-line-strong p-3">
          <p id={warningId} className="font-semibold">{t("keepMeetingRecordingsQuestion")}</p>
          <p className="text-sm">{t("keepRecordingsWarning")}</p>
          <div className="flex flex-wrap gap-2">
            <button type="button" className={buttonClass} onClick={() => keep(true)}>{t("keepRecordings")}</button>
            <button type="button" className={buttonClass} onClick={() => setWarning(false)}>{t("cancel")}</button>
          </div>
        </div>
      ) : null}
      {status === null ? null : (
        <p>{t("recordingsKept", { count: status.count, size: recordingsSize(status.bytes) })}</p>
      )}
      {confirmWipe ? (
        <div role="alertdialog" aria-label={t("wipeAllRecordingsQuestion")} className="space-y-2 rounded-lg border border-line-strong p-3">
          <p className="font-semibold">{t("wipeAllRecordingsQuestion")}</p>
          <p className="text-sm">{t("wipeAllRecordingsExplanation")}</p>
          <div className="flex flex-wrap gap-2">
            <button type="button" className={`${buttonClass} text-danger`} onClick={wipe}>{t("wipeAllRecordings")}</button>
            <button type="button" className={buttonClass} onClick={() => setConfirmWipe(false)}>{t("cancel")}</button>
          </div>
        </div>
      ) : (
        <button
          type="button"
          className={`${buttonClass} text-danger`}
          disabled={status === null || status.count === 0 || busy}
          onClick={() => setConfirmWipe(true)}
        >
          {t("wipeAllRecordings")}
        </button>
      )}
      <p role="status" aria-live="polite">{message}</p>
    </div>
  );
}
