// Renders a meeting's notes on its page (#96): the owner's Markdown, shown as written, and a text
// field to write or change them. Saved notes go into the meeting, which syncs to paired devices,
// and summaries read them.

"use client";

import { useId, useState } from "react";
import { editErrorMessage, saveMeetingNotes } from "@/lib/edits-api";
import { useT } from "@/lib/i18n/client";

const buttonClass =
  "inline-flex min-h-11 items-center justify-center rounded-lg border border-line-strong px-4 hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus disabled:text-faint";
const fieldClass =
  "min-h-40 w-full rounded-lg border border-line-strong bg-surface px-3 py-2 font-mono text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30";

/**
 * Shows and edits a meeting's notes.
 *
 * Inputs: the meeting and its notes (`null` when there are none). Outputs: the notes, or a text
 * field while editing, and a status line. Side effects: "Save notes" writes them to the meeting.
 */
export function MeetingNotes({ meetingId, notes }: { meetingId: string; notes: string | null }) {
  const t = useT();
  const [current, setCurrent] = useState(notes);
  const [editing, setEditing] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const fieldId = useId();

  /** Saves the text being edited. */
  async function save(text: string) {
    setBusy(true);
    setMessage(null);
    try {
      await saveMeetingNotes(meetingId, text);
      setCurrent(text);
      setEditing(null);
      setMessage(t("notesSaved"));
    } catch (error) {
      setMessage(editErrorMessage(error, t));
    } finally {
      setBusy(false);
    }
  }

  const hasNotes = current !== null && current.trim().length > 0;
  return (
    <section id="notes" aria-labelledby={`${meetingId}-notes-heading`} className="card scroll-mt-20 space-y-3">
      <h2 id={`${meetingId}-notes-heading`}>{t("notes")}</h2>
      {editing === null ? (
        <>
          {hasNotes ? (
            <div className="whitespace-pre-wrap break-words">{current}</div>
          ) : (
            <p className="text-muted">{t("noNotesYet")}</p>
          )}
          <button type="button" className={buttonClass} onClick={() => setEditing(current ?? "")}>
            {hasNotes ? t("editNotes") : t("writeNotes")}
          </button>
        </>
      ) : (
        <div className="space-y-2">
          <label htmlFor={fieldId} className="block font-semibold">{t("notesMarkdown")}</label>
          <textarea id={fieldId} className={fieldClass} value={editing} onChange={(event) => setEditing(event.target.value)} />
          <div className="flex flex-wrap gap-2">
            <button type="button" className={buttonClass} disabled={busy} onClick={() => void save(editing)}>{t("saveNotes")}</button>
            <button type="button" className={buttonClass} disabled={busy} onClick={() => setEditing(null)}>{t("cancel")}</button>
          </div>
        </div>
      )}
      <p role="status" aria-live="polite">
        {message}
      </p>
    </section>
  );
}
