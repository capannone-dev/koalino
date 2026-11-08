// Lists meetings with a per-row actions menu (button or right-click) that renames a meeting in place,
// adds it to a project (#94), or deletes it.

"use client";

import Link from "next/link";
import { KeyboardEvent, useEffect, useRef, useState } from "react";
import { ConfirmDeleteDialog } from "@/components/confirm-delete-dialog";
import { MeetingProjectDialog } from "@/components/meeting-project-dialog";
import { RenameMeetingDialog } from "@/components/rename-meeting-dialog";
import type { MeetingSummary } from "@/lib/api";
import { deleteMeetingWithMessage, meetingDeletionMessage } from "@/lib/deletion-messages";
import { describeRecordedOn } from "@/lib/meeting-metadata";

/** The row whose actions menu is open; `position` is set when it was opened by right-click. */
type OpenMenu = { meetingId: string; position: { x: number; y: number } | null };

/**
 * Renders `meetings` as links to their detail pages, each with an actions menu offering Rename,
 * Add to project… and Delete.
 *
 * Inputs: the server-rendered meeting summaries. Outputs: the list, its menus, the rename dialog,
 * and a polite status region announcing "Meeting renamed". Side effects: renames through the
 * dialog and replaces the row's title from the server response, without reloading the page.
 */
export function MeetingList({ meetings }: { meetings: MeetingSummary[] }) {
  const [currentMeetings, setCurrentMeetings] = useState(meetings);
  const [openMenu, setOpenMenu] = useState<OpenMenu | null>(null);
  const [renamingMeeting, setRenamingMeeting] = useState<MeetingSummary | null>(null);
  const [deletingMeeting, setDeletingMeeting] = useState<MeetingSummary | null>(null);
  const [movingMeeting, setMovingMeeting] = useState<MeetingSummary | null>(null);
  const [statusMessage, setStatusMessage] = useState("");
  // Each row's actions button, so focus can return to it after the menu or dialog closes.
  const actionButtons = useRef(new Map<string, HTMLButtonElement>());
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!openMenu) return;
    menuRef.current?.querySelector<HTMLElement>("[role=menuitem]")?.focus();
    /** Closes the menu when the person clicks anywhere outside it. */
    function closeOnOutsidePointer(event: MouseEvent) {
      if (!menuRef.current?.contains(event.target as Node)) setOpenMenu(null);
    }
    document.addEventListener("mousedown", closeOnOutsidePointer);
    return () => document.removeEventListener("mousedown", closeOnOutsidePointer);
  }, [openMenu]);

  /** Moves focus back to `meetingId`'s actions button. */
  function focusActionsButton(meetingId: string) {
    actionButtons.current.get(meetingId)?.focus();
  }

  /** Closes the menu on Escape or Tab, returning focus to the actions button on Escape. */
  function handleMenuKeyDown(event: KeyboardEvent<HTMLDivElement>, meetingId: string) {
    if (event.key === "Escape") {
      event.preventDefault();
      setOpenMenu(null);
      focusActionsButton(meetingId);
    } else if (event.key === "Tab") {
      setOpenMenu(null);
    }
  }

  /** Closes the rename dialog and returns focus to the row that opened it. */
  function closeDialog(meetingId: string) {
    setRenamingMeeting(null);
    focusActionsButton(meetingId);
  }

  if (currentMeetings.length === 0) {
    return (
      <>
        <p role="status" aria-live="polite" className="text-sm text-success">
          {statusMessage}
        </p>
        <p className="text-muted">No meetings yet.</p>
      </>
    );
  }

  return (
    <>
      <p role="status" aria-live="polite" className="text-sm text-success">
        {statusMessage}
      </p>
      <ul className="divide-y divide-line rounded-xl border border-line bg-surface">
        {currentMeetings.map((meeting) => {
          const isMenuOpen = openMenu?.meetingId === meeting.meeting_id;
          const menuId = `meeting-actions-${meeting.meeting_id}`;
          return (
            <li
              key={meeting.meeting_id}
              className="relative flex items-center gap-2 px-3 py-2 sm:px-4"
              onContextMenu={(event) => {
                event.preventDefault();
                setOpenMenu({ meetingId: meeting.meeting_id, position: { x: event.clientX, y: event.clientY } });
              }}
            >
              <div className="min-w-0 flex-1 py-1">
                <Link
                  href={`/meetings/${encodeURIComponent(meeting.meeting_id)}`}
                  className="block font-semibold break-words text-ink hover:underline"
                >
                  {meeting.title}
                </Link>
                <small className="block truncate text-muted">{meeting.meeting_id}</small>
                {describeRecordedOn(meeting) === null ? null : (
                  <small className="block truncate text-muted">{describeRecordedOn(meeting)}</small>
                )}
              </div>
              <button
                ref={(button) => {
                  if (button) actionButtons.current.set(meeting.meeting_id, button);
                  else actionButtons.current.delete(meeting.meeting_id);
                }}
                type="button"
                aria-label={`Actions for ${meeting.title}`}
                aria-haspopup="menu"
                aria-expanded={isMenuOpen}
                aria-controls={isMenuOpen ? menuId : undefined}
                onClick={() => setOpenMenu(isMenuOpen ? null : { meetingId: meeting.meeting_id, position: null })}
                className="inline-flex min-h-11 min-w-11 shrink-0 items-center justify-center rounded-full text-xl text-muted hover:bg-surface-muted focus-visible:outline-2 focus-visible:outline-focus"
              >
                <span aria-hidden="true">⋯</span>
              </button>
              {isMenuOpen ? (
                <div
                  ref={menuRef}
                  id={menuId}
                  role="menu"
                  aria-label={`Actions for ${meeting.title}`}
                  onKeyDown={(event) => handleMenuKeyDown(event, meeting.meeting_id)}
                  style={openMenu.position ? { left: openMenu.position.x, top: openMenu.position.y } : undefined}
                  className={`z-40 min-w-40 rounded-lg border border-line bg-surface py-1 shadow-lg ${
                    openMenu.position ? "fixed" : "absolute top-full right-3"
                  }`}
                >
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setOpenMenu(null);
                      setStatusMessage("");
                      setRenamingMeeting(meeting);
                    }}
                    className="flex min-h-11 w-full items-center px-4 text-left hover:bg-surface-muted focus:bg-surface-muted focus:outline-none"
                  >
                    Rename
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setOpenMenu(null);
                      setStatusMessage("");
                      setMovingMeeting(meeting);
                    }}
                    className="flex min-h-11 w-full items-center px-4 text-left hover:bg-surface-muted focus:bg-surface-muted focus:outline-none"
                  >
                    Add to project…
                  </button>
                  <button
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      setOpenMenu(null);
                      setStatusMessage("");
                      setDeletingMeeting(meeting);
                    }}
                    className="flex min-h-11 w-full items-center px-4 text-left text-danger hover:bg-surface-muted"
                  >
                    Delete
                  </button>
                </div>
              ) : null}
            </li>
          );
        })}
      </ul>
      {deletingMeeting ? (
        <ConfirmDeleteDialog
          title={`Delete ${deletingMeeting.title}?`}
          message={meetingDeletionMessage}
          onClose={() => {
            focusActionsButton(deletingMeeting.meeting_id);
            setDeletingMeeting(null);
          }}
          onConfirm={async () => {
            await deleteMeetingWithMessage(deletingMeeting.meeting_id);
            setCurrentMeetings((current) =>
              current.filter((candidate) => candidate.meeting_id !== deletingMeeting.meeting_id)
            );
            setStatusMessage("Meeting deleted");
            setDeletingMeeting(null);
          }}
        />
      ) : null}
      {movingMeeting ? (
        <MeetingProjectDialog
          meeting={movingMeeting}
          onClose={() => {
            focusActionsButton(movingMeeting.meeting_id);
            setMovingMeeting(null);
          }}
        />
      ) : null}
      {renamingMeeting ? (
        <RenameMeetingDialog
          meeting={renamingMeeting}
          onClose={() => closeDialog(renamingMeeting.meeting_id)}
          onRenamed={(summary) => {
            setCurrentMeetings((current) =>
              current.map((candidate) => (candidate.meeting_id === summary.meeting_id ? summary : candidate))
            );
            setStatusMessage("Meeting renamed");
            closeDialog(summary.meeting_id);
          }}
        />
      ) : null}
    </>
  );
}
