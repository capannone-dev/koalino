// Asks the owner to confirm an irreversible deletion, for meetings and voices alike.

"use client";

import { KeyboardEvent, useEffect, useId, useRef, useState } from "react";
import { useT } from "@/lib/i18n/client";

export type ConfirmDeleteDialogProps = {
  /** The question, naming what goes. */
  title: string;
  /** What the deletion means. */
  message: string;
  /** Deletes the item; a thrown error's text is shown and the dialog stays open. */
  onConfirm: () => Promise<void>;
  /** Called on Cancel, Escape or a click outside. */
  onClose: () => void;
};

/**
 * Shows `title` and `message` with Cancel and Delete, focused on Cancel so Enter cannot delete.
 *
 * Inputs: see `ConfirmDeleteDialogProps`. Outputs: a modal dialog. Side effects: runs `onConfirm`
 * on Delete. Callers unmount it once `onConfirm` resolves.
 */
export function ConfirmDeleteDialog({ title, message, onConfirm, onClose }: ConfirmDeleteDialogProps) {
  const t = useT();
  const [isDeleting, setIsDeleting] = useState(false);
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const cancelRef = useRef<HTMLButtonElement>(null);
  const deleteRef = useRef<HTMLButtonElement>(null);
  const headingId = useId();
  const messageId = useId();

  useEffect(() => {
    cancelRef.current?.focus();
  }, []);

  /** Runs the deletion, keeping the dialog open with the reason when it fails. */
  async function confirm() {
    setIsDeleting(true);
    setErrorMessage(null);
    try {
      await onConfirm();
    } catch (error) {
      setErrorMessage(error instanceof Error ? error.message : t("couldntDeleteThis"));
      setIsDeleting(false);
    }
  }

  /** Closes on Escape and keeps Tab between the two buttons. */
  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape" && !isDeleting) {
      event.preventDefault();
      onClose();
    } else if (event.key === "Tab") {
      event.preventDefault();
      (document.activeElement === cancelRef.current ? deleteRef : cancelRef).current?.focus();
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-slate-900/50 p-4 sm:items-center"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget && !isDeleting) onClose();
      }}
    >
      <div
        role="alertdialog"
        aria-modal="true"
        aria-labelledby={headingId}
        aria-describedby={messageId}
        onKeyDown={handleKeyDown}
        className="w-full max-w-md rounded-2xl bg-surface p-6 shadow-xl"
      >
        <h2 id={headingId} className="text-lg font-semibold break-words">
          {title}
        </h2>
        <p id={messageId} className="mt-2 text-ink-soft">
          {message}
        </p>
        <p role="alert" className="mt-2 text-sm text-danger">
          {errorMessage}
        </p>
        <div className="mt-6 flex flex-col-reverse gap-2 sm:flex-row sm:justify-end">
          <button
            ref={cancelRef}
            type="button"
            disabled={isDeleting}
            onClick={onClose}
            className="min-h-11 rounded-lg px-4 font-medium text-accent-text hover:bg-accent-soft disabled:text-faint"
          >{t("cancel")}</button>
          <button
            ref={deleteRef}
            type="button"
            disabled={isDeleting}
            onClick={confirm}
            className="min-h-11 rounded-lg bg-danger px-4 font-medium text-on-accent hover:opacity-90 disabled:opacity-50"
          >{t("delete")}</button>
        </div>
      </div>
    </div>
  );
}
