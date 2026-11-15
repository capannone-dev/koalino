// Renders the web Settings page's log controls (Issue #46): the daemon's log level and a download
// of its logs. The level lasts until the daemon restarts; its start level comes from RUST_LOG.

"use client";

import { ChangeEvent, useId, useState } from "react";
import { fetchLogExport, LogSettingsError, setLogLevel, type LogSettingsFailureKind } from "@/lib/api";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";

/** The levels offered, from least to most detail. */
const logLevels = (t: Translate) => [
  { value: "error", label: "Error" },
  { value: "warn", label: t("warning") },
  { value: "info", label: t("infoDefault") },
  { value: "debug", label: t("debug") },
  { value: "trace", label: t("trace") }
] as const;

/** Plain messages per failure; server text is never shown because it can contain internal details. */
const failureMessages = (t: Translate) : Record<LogSettingsFailureKind, string> => ({
  invalid_level: t("theServerRefusedThisLog"),
  no_logs: t("thereAreNoLogsTo"),
  unavailable: t("thisServerHasNoLog"),
  server: t("somethingWentWrongOnThe"),
  network: t("cantReachTheNoteTaking")
});

/** Words any thrown value as a failure message. */
function failureMessage(t: Translate, error: unknown): string {
  return failureMessages(t)[error instanceof LogSettingsError ? error.kind : "server"];
}

/** Saves a downloaded file through a temporary link, the way the browser saves any download. */
export function saveBlobAsFile(blob: Blob, fileName: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = fileName;
  link.click();
  URL.revokeObjectURL(url);
}

export type LogSettingsProps = {
  /** The level in effect, or `null` when it could not be read. */
  initialLevel: string | null;
  /** Saves the downloaded logs; tests replace it. */
  saveFile?: (blob: Blob, fileName: string) => void;
};

/**
 * Lets the owner change the daemon's log level and download its logs.
 *
 * Inputs: see `LogSettingsProps`. Outputs: a level selector, an Export logs button, and a status
 * line (announced to screen readers). Side effects: `PUT /api/settings/log-level` on a change and
 * `GET /api/logs/export` on Export logs, which saves the file.
 */
export function LogSettings({ initialLevel, saveFile = saveBlobAsFile }: LogSettingsProps) {
  const t = useT();
  const [level, setLevel] = useState(initialLevel);
  const [isSaving, setIsSaving] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initialLevel === null ? t("cantReadTheLogLevel") : null
  );
  const selectId = useId();
  const isOffered = logLevels(t).some((offered) => offered.value === level);

  /** Applies the chosen level, keeping the old one when the server refuses it. */
  async function changeLevel(event: ChangeEvent<HTMLSelectElement>) {
    const chosen = event.target.value;
    setIsSaving(true);
    setMessage(null);
    try {
      const applied = await setLogLevel(chosen);
      setLevel(applied);
      const label = logLevels(t).find((offered) => offered.value === applied)?.label.replace(" (default)", "") ?? applied;
      setMessage(t("theServerNowLogsAt", { label }));
    } catch (error) {
      setMessage(failureMessage(t, error));
    } finally {
      setIsSaving(false);
    }
  }

  /** Downloads the logs and saves them. */
  async function exportLogs() {
    setIsExporting(true);
    setMessage(null);
    try {
      const download = await fetchLogExport();
      saveFile(download.blob, download.fileName);
      setMessage(t("downloaded", { fileName: download.fileName }));
    } catch (error) {
      setMessage(failureMessage(t, error));
    } finally {
      setIsExporting(false);
    }
  }

  return (
    <div className="card max-w-2xl space-y-6">
      <div className="space-y-1">
        <label htmlFor={selectId} className="block font-semibold">{t("logLevel")}</label>
        <p className="text-sm text-muted">{t("howMuchDetailTheServer")}</p>
        <select
          id={selectId}
          className="block min-h-11 rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 disabled:text-faint"
          value={level ?? ""}
          disabled={level === null || isSaving}
          onChange={changeLevel}
        >
          {level !== null && !isOffered && <option value={level}>{t("custom", { level })}</option>}
          {logLevels(t).map((offered) => (
            <option key={offered.value} value={offered.value}>
              {offered.label}
            </option>
          ))}
        </select>
      </div>
      <div className="space-y-1">
        <p className="font-semibold">{t("logs")}</p>
        <p className="text-sm text-muted">{t("theLast7DaysOf")}</p>
        <button type="button" className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled" disabled={isExporting} onClick={exportLogs}>
          {isExporting ? t("exporting") : t("exportLogs")}
        </button>
      </div>
      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}
