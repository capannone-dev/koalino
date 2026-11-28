// Renders the web Settings page's notes folder (#98): plain files on the daemon's computer that
// mirror projects and meetings, with the warning that they are a decrypted copy.

"use client";

import { useId, useState } from "react";
import {
  NotesFolderError,
  notesFolderMessage,
  setNotesFolder,
  syncNotesFolder,
  type NotesFolderStatus
} from "@/lib/notes-folder-api";
import { useT } from "@/lib/i18n/client";
import type { Translate } from "@/lib/i18n";
import { buttonStyles } from "@/components/ui";

export type NotesFolderSettingsProps = {
  /** The saved status, or `null` when it could not be read. */
  initial: NotesFolderStatus | null;
};

/** Words the last round, or `null` before one. */
function lastRound(t: Translate, status: NotesFolderStatus): string | null {
  if (!status.last_round_at) {
    return null;
  }
  const at = new Date(status.last_round_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return status.last_error
    ? `Last sync at ${at} failed: ${notesFolderMessage(status.last_error, t)}`
    : t("lastSyncAtFilesWritten", { at, written: status.written, imported: status.imported });
}

/**
 * Lets the owner keep a notes folder on the daemon's computer.
 *
 * Inputs: see `NotesFolderSettingsProps`. Outputs: the folder field, a switch, Sync now while on,
 * and a status line announced to screen readers. Side effects: `PUT /api/settings/notes-folder`
 * on the switch, `POST /api/settings/notes-folder/sync` on Sync now.
 */
export function NotesFolderSettings({ initial }: NotesFolderSettingsProps) {
  const t = useT();
  const [status, setStatus] = useState<NotesFolderStatus | null>(initial);
  const [path, setPath] = useState(initial?.path ?? initial?.suggested_path ?? "");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? t("cantReadTheNotesFolder") : null
  );
  const fieldId = useId();
  const hintId = useId();

  async function run(action: () => Promise<NotesFolderStatus>) {
    setBusy(true);
    setMessage(null);
    try {
      const saved = await action();
      setStatus(saved);
      if (saved.path) {
        setPath(saved.path);
      }
      setMessage(lastRound(t, saved));
    } catch (error) {
      setMessage(notesFolderMessage(error instanceof NotesFolderError ? error.code : "server", t));
    } finally {
      setBusy(false);
    }
  }

  const enabled = status?.enabled ?? false;
  return (
    <div className="card max-w-2xl space-y-3">
      <div className="space-y-1">
        <h2 className="font-semibold">{t("notesFolder")}</h2>
        <p id={hintId} className="text-sm text-muted">{t("keepProjectsAndMeetingsAs")}</p>
        <p className="rounded-lg border border-line-strong p-2 text-sm">{t("theFolderIsADecrypted")}</p>
      </div>
      <label htmlFor={fieldId} className="block text-sm font-semibold">{t("folderOnTheServer")}</label>
      <input
        id={fieldId}
        aria-describedby={hintId}
        className="min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30"
        value={path}
        disabled={status === null || busy}
        onChange={(event) => setPath(event.target.value)}
      />
      <div className="flex flex-wrap gap-2">
        <label className="flex min-h-11 items-center gap-2">
          <input
            type="checkbox"
            role="switch"
            checked={enabled}
            disabled={status === null || busy}
            onChange={(event) => void run(() => setNotesFolder(event.target.checked, path.trim() || null))}
          />{t("keepTheNotesFolder")}</label>
        {enabled ? (
          <button type="button" className={buttonStyles.filled}
            disabled={busy}
            onClick={() => void run(syncNotesFolder)}>{t("syncNow")}</button>
        ) : null}
      </div>
      <p role="status" aria-live="polite">
        {message ?? (status ? lastRound(t, status) : null)}
      </p>
    </div>
  );
}
