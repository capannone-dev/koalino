// Renders a meeting detail page's title heading with a pencil button that renames the meeting.

"use client";

import { useRouter } from "next/navigation";
import { useRef, useState } from "react";
import { ConfirmDeleteDialog } from "@/components/confirm-delete-dialog";
import { RenameMeetingDialog } from "@/components/rename-meeting-dialog";
import type { MeetingSummary } from "@/lib/api";
import { deleteMeetingWithMessage, meetingDeletionMessage } from "@/lib/deletion-messages";
import { useT } from "@/lib/i18n/client";

/**
 * Shows `meeting.title` as the page's `h1`, with a pencil button opening the rename dialog.
 *
 * Inputs: the server-rendered meeting summary. Outputs: the heading, the button, and a polite
 * status region announcing "Meeting renamed". Side effects: renames through the dialog and shows
 * the new title immediately, without reloading the page.
 */
export function MeetingTitleHeader({ meeting }: { meeting: MeetingSummary }) {
  const t = useT();
  const [currentMeeting, setCurrentMeeting] = useState(meeting);
  const [isRenaming, setIsRenaming] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);
  const router = useRouter();
  const [statusMessage, setStatusMessage] = useState("");
  const pencilRef = useRef<HTMLButtonElement>(null);

  /** Closes the dialog and returns focus to the pencil. */
  function closeDialog() {
    setIsRenaming(false);
    pencilRef.current?.focus();
  }

  return (
    <>
      <div className="flex items-start gap-2">
        <h1 className="min-w-0 text-2xl font-bold break-words sm:text-3xl">{currentMeeting.title}</h1>
        <button
          ref={pencilRef}
          type="button"
          aria-label={t("renameMeeting")}
          title={t("renameMeeting")}
          onClick={() => {
            setStatusMessage("");
            setIsRenaming(true);
          }}
          className="inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center rounded-full text-muted hover:bg-surface-hover focus-visible:outline-2 focus-visible:outline-focus"
        >
          <PencilIcon />
        </button>
        <button
          type="button"
          aria-label={t("deleteMeeting")}
          title={t("deleteMeeting")}
          onClick={() => setIsDeleting(true)}
          className="inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center rounded-lg text-danger hover:bg-surface-muted"
        >
          <TrashIcon />
        </button>
      </div>
      <p role="status" aria-live="polite" className="text-sm text-success">
        {statusMessage}
      </p>
      {isDeleting ? (
        <ConfirmDeleteDialog
          title={t("delete2", { title: currentMeeting.title })}
          message={meetingDeletionMessage(t)}
          onClose={() => setIsDeleting(false)}
          onConfirm={async () => {
            await deleteMeetingWithMessage(currentMeeting.meeting_id, t);
            router.push("/meetings");
            router.refresh();
          }}
        />
      ) : null}
      {isRenaming ? (
        <RenameMeetingDialog
          meeting={currentMeeting}
          onClose={closeDialog}
          onRenamed={(summary) => {
            setCurrentMeeting(summary);
            setStatusMessage(t("meetingRenamed"));
            closeDialog();
          }}
        />
      ) : null}
    </>
  );
}

/** A 20 px outline bin, hidden from assistive technology because the button is labelled. */
function TrashIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M3 6h18" />
      <path d="M8 6V4h8v2" />
      <path d="M19 6l-1 14H6L5 6" />
    </svg>
  );
}

/** A 20 px outline pencil, hidden from assistive technology because the button is labelled. */
function PencilIcon() {
  return (
    <svg aria-hidden="true" viewBox="0 0 24 24" width="20" height="20" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
      <path d="M12 20h9" />
      <path d="M16.5 3.5a2.1 2.1 0 0 1 3 3L7 19l-4 1 1-4Z" />
    </svg>
  );
}
