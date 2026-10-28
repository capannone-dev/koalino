// Renders the web Settings page's log controls (Issue #46): the daemon's log level and a download
// of its logs. The level lasts until the daemon restarts; its start level comes from RUST_LOG.

"use client";

import { ChangeEvent, useId, useState } from "react";
import { fetchLogExport, LogSettingsError, setLogLevel, type LogSettingsFailureKind } from "@/lib/api";

/** The levels offered, from least to most detail. */
const logLevels = [
  { value: "error", label: "Error" },
  { value: "warn", label: "Warning" },
  { value: "info", label: "Info (default)" },
  { value: "debug", label: "Debug" },
  { value: "trace", label: "Trace" }
] as const;

/** Plain messages per failure; server text is never shown because it can contain internal details. */
const failureMessages: Record<LogSettingsFailureKind, string> = {
  invalid_level: "The server refused this log level.",
  no_logs: "There are no logs to export yet.",
  unavailable: "This server has no log settings.",
  server: "Something went wrong on the server. Try again.",
  network: "Can't reach the note-taking server. Check that it is running."
};

/** Words any thrown value as a failure message. */
function failureMessage(error: unknown): string {
  return failureMessages[error instanceof LogSettingsError ? error.kind : "server"];
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
  const [level, setLevel] = useState(initialLevel);
  const [isSaving, setIsSaving] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initialLevel === null ? "Can't read the log level. Check that the note-taking server is running." : null
  );
  const selectId = useId();
  const isOffered = logLevels.some((offered) => offered.value === level);

  /** Applies the chosen level, keeping the old one when the server refuses it. */
  async function changeLevel(event: ChangeEvent<HTMLSelectElement>) {
    const chosen = event.target.value;
    setIsSaving(true);
    setMessage(null);
    try {
      const applied = await setLogLevel(chosen);
      setLevel(applied);
      const label = logLevels.find((offered) => offered.value === applied)?.label.replace(" (default)", "") ?? applied;
      setMessage(`The server now logs at ${label} until it restarts.`);
    } catch (error) {
      setMessage(failureMessage(error));
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
      setMessage(`Downloaded ${download.fileName}.`);
    } catch (error) {
      setMessage(failureMessage(error));
    } finally {
      setIsExporting(false);
    }
  }

  return (
    <div className="card max-w-2xl space-y-6">
      <div className="space-y-1">
        <label htmlFor={selectId} className="block font-semibold">
          Log level
        </label>
        <p className="text-sm text-muted">How much detail the server writes to its logs. Debug and Trace help diagnose problems but write more.</p>
        <select
          id={selectId}
          className="block min-h-11 rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 disabled:text-faint"
          value={level ?? ""}
          disabled={level === null || isSaving}
          onChange={changeLevel}
        >
          {level !== null && !isOffered && <option value={level}>{`Custom (${level})`}</option>}
          {logLevels.map((offered) => (
            <option key={offered.value} value={offered.value}>
              {offered.label}
            </option>
          ))}
        </select>
      </div>
      <div className="space-y-1">
        <p className="font-semibold">Logs</p>
        <p className="text-sm text-muted">The last 7 days of the server&apos;s diagnostic logs, for example to attach to a bug report.</p>
        <button type="button" className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled" disabled={isExporting} onClick={exportLogs}>
          {isExporting ? "Exporting…" : "Export logs"}
        </button>
      </div>
      <p role="status" aria-live="polite">
        {message}
      </p>
    </div>
  );
}
