// Renders the modal that renames a meeting, shared by the meeting list and the meeting detail header.

"use client";

import { FormEvent, KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { MeetingRenameError, renameMeeting, type MeetingRenameFailureKind, type MeetingSummary } from "@/lib/api";
import { maxMeetingTitleCharacters, validateMeetingTitle } from "@/lib/meeting-title";

/** Plain messages per failure; server text is never shown because it can contain internal details. */
const renameFailureMessages: Record<MeetingRenameFailureKind, string> = {
  invalid_title: "This title isn't allowed.",
  not_found: "This meeting no longer exists.",
  server: "Couldn't save the new title. Try again.",
  network: "Can't reach the note-taking server. Check that it is running.",
  unexpected: "Couldn't rename the meeting."
};

export type RenameMeetingDialogProps = {
  /** The meeting being renamed; its title pre-fills the field. */
  meeting: MeetingSummary;
  /** Called with the server's updated summary after a successful rename. */
  onRenamed: (summary: MeetingSummary) => void;
  /** Called when the dialog closes without a rename (Cancel, Escape, backdrop, unchanged title). */
  onClose: () => void;
};

/**
 * Lets the person edit a meeting title, validates it like Rust, and saves it through the API.
 *
 * Inputs: see `RenameMeetingDialogProps`. Outputs: a modal dialog; it keeps focus inside itself and
 * stays open with a message when validation or the request fails. Side effects: sends
 * `PATCH /api/meetings/{id}` when a new valid title is saved. Callers unmount it on either callback
 * and restore focus to the control that opened it.
 */
export function RenameMeetingDialog({ meeting, onRenamed, onClose }: RenameMeetingDialogProps) {
  const [typedTitle, setTypedTitle] = useState(meeting.title);
  const [hasAttemptedSave, setHasAttemptedSave] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [saveErrorMessage, setSaveErrorMessage] = useState<string | null>(null);
  const dialogRef = useRef<HTMLDivElement>(null);
  const titleInputRef = useRef<HTMLInputElement>(null);
  const headingId = useId();
  const errorId = useId();
  const counterId = useId();

  useEffect(() => {
    titleInputRef.current?.focus();
    titleInputRef.current?.select();
  }, []);

  const validation = validateMeetingTitle(typedTitle);
  // Validation errors appear after the first Save, so clearing the field to retype does not flash one.
  const errorMessage = (hasAttemptedSave ? validation.error : null) ?? saveErrorMessage;
  const characterCount = Array.from(validation.trimmedTitle).length;

  /** Validates the field and, when it holds a new valid title, renames the meeting. */
  async function save(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (isSaving) return;
    setHasAttemptedSave(true);
    setSaveErrorMessage(null);
    if (validation.error !== null) return;
    if (validation.trimmedTitle === meeting.title) {
      onClose();
      return;
    }
    setIsSaving(true);
    try {
      onRenamed(await renameMeeting(meeting.meeting_id, validation.trimmedTitle));
    } catch (error) {
      setSaveErrorMessage(renameFailureMessages[error instanceof MeetingRenameError ? error.kind : "unexpected"]);
      setIsSaving(false);
    }
  }

  /** Closes on Escape and keeps Tab and Shift+Tab cycling inside the dialog. */
  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      if (!isSaving) onClose();
      return;
    }
    if (event.key !== "Tab" || !dialogRef.current) return;
    const focusable = Array.from(dialogRef.current.querySelectorAll<HTMLElement>("input, button:not([disabled])"));
    const first = focusable[0];
    const last = focusable[focusable.length - 1];
    if (event.shiftKey && document.activeElement === first) {
      event.preventDefault();
      last?.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first?.focus();
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-slate-900/50 p-4 sm:items-center"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !isSaving) onClose();
      }}
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={headingId}
        onKeyDown={handleKeyDown}
        className="w-full max-w-md rounded-2xl bg-surface p-6 shadow-xl"
      >
        <h2 id={headingId} className="text-lg font-semibold">
          Rename meeting
        </h2>
        <form onSubmit={save} noValidate className="mt-4">
          <label htmlFor={`${headingId}-title`} className="block text-sm font-medium text-ink-soft">
            Title
          </label>
          <input
            ref={titleInputRef}
            id={`${headingId}-title`}
            type="text"
            value={typedTitle}
            disabled={isSaving}
            aria-invalid={errorMessage !== null}
            aria-describedby={errorMessage !== null ? `${errorId} ${counterId}` : counterId}
            onChange={(event) => {
              setTypedTitle(event.target.value);
              setSaveErrorMessage(null);
            }}
            className="mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 aria-invalid:border-danger"
          />
          <div className="mt-1 flex items-start justify-between gap-4 text-sm">
            <p id={errorId} role="alert" className="text-danger">
              {errorMessage}
            </p>
            {/* Counted in Unicode characters, as Rust counts them. */}
            <p id={counterId} className="shrink-0 text-muted">
              {characterCount}/{maxMeetingTitleCharacters}
            </p>
          </div>
          <div className="mt-6 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
            <button
              type="button"
              disabled={isSaving}
              onClick={onClose}
              className="min-h-11 rounded-lg px-4 font-medium text-accent-text hover:bg-accent-soft disabled:text-faint"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={isSaving}
              className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled"
            >
              {isSaving ? "Saving…" : "Save"}
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
