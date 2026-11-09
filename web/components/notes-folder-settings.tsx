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

export type NotesFolderSettingsProps = {
  /** The saved status, or `null` when it could not be read. */
  initial: NotesFolderStatus | null;
};

/** Words the last round, or `null` before one. */
function lastRound(status: NotesFolderStatus): string | null {
  if (!status.last_round_at) {
    return null;
  }
  const at = new Date(status.last_round_at).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
  return status.last_error
    ? `Last sync at ${at} failed: ${notesFolderMessage(status.last_error)}`
    : `Last sync at ${at}: ${status.written} files written, ${status.imported} edits and files taken in.`;
}

/**
 * Lets the owner keep a notes folder on the daemon's computer.
 *
 * Inputs: see `NotesFolderSettingsProps`. Outputs: the folder field, a switch, Sync now while on,
 * and a status line announced to screen readers. Side effects: `PUT /api/settings/notes-folder`
 * on the switch, `POST /api/settings/notes-folder/sync` on Sync now.
 */
export function NotesFolderSettings({ initial }: NotesFolderSettingsProps) {
  const [status, setStatus] = useState<NotesFolderStatus | null>(initial);
  const [path, setPath] = useState(initial?.path ?? initial?.suggested_path ?? "");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(
    initial === null ? "Can't read the notes folder setting. Check that the note-taking server is running." : null
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
      setMessage(lastRound(saved));
    } catch (error) {
      setMessage(notesFolderMessage(error instanceof NotesFolderError ? error.code : "server"));
    } finally {
      setBusy(false);
    }
  }

  const enabled = status?.enabled ?? false;
  return (
    <div className="card max-w-2xl space-y-3">
      <div className="space-y-1">
        <h2 className="font-semibold">Notes folder</h2>
        <p id={hintId} className="text-sm text-muted">
          Keep projects and meetings as plain files on the server&apos;s computer, to read and edit them with any
          editor. Edits to notes.md, summary.md and project files come back within half a minute, and files dropped
          into a project folder or a meeting&apos;s attachments folder are added. transcript.md is rewritten: correct
          the transcript here.
        </p>
        <p className="rounded-lg border border-line-strong p-2 text-sm">
          The folder is a decrypted copy of your notes: anyone who can read it can read them, and backup or sync tools
          will copy it. Keep it on an encrypted disk, inside your own user folder.
        </p>
      </div>
      <label htmlFor={fieldId} className="block text-sm font-semibold">
        Folder on the server
      </label>
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
          />
          Keep the notes folder
        </label>
        {enabled ? (
          <button type="button" className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled"
            disabled={busy}
            onClick={() => void run(syncNotesFolder)}>
            Sync now
          </button>
        ) : null}
      </div>
      <p role="status" aria-live="polite">
        {message ?? (status ? lastRound(status) : null)}
      </p>
    </div>
  );
}
