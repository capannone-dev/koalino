// Renders the modal the meeting list opens from a row's "Add to project…" (#94): it loads the
// projects and shows the meeting's project picker.

"use client";

import { KeyboardEvent, useEffect, useId, useState } from "react";
import { MeetingProjectPicker } from "@/components/meeting-project-picker";
import type { MeetingSummary } from "@/lib/api";
import { getProjects, projectErrorMessage, type ProjectSummary } from "@/lib/projects-api";
import { useT } from "@/lib/i18n/client";
import { buttonStyles } from "@/components/ui";

/**
 * Lets the person choose the project of one meeting from the list.
 *
 * Inputs: the meeting and `onClose`. Outputs: a modal dialog holding the picker, or a plain
 * message when the projects cannot be read. Side effects: `GET /api/projects` when it opens;
 * the picker moves the meeting. Closes on Escape, the backdrop, or Done.
 */
export function MeetingProjectDialog({ meeting, onClose }: { meeting: MeetingSummary; onClose: () => void }) {
  const t = useT();
  const [projects, setProjects] = useState<ProjectSummary[] | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const headingId = useId();

  useEffect(() => {
    let isCurrent = true;
    getProjects()
      .then((loaded) => {
        if (isCurrent) setProjects(loaded);
      })
      .catch((error: unknown) => {
        if (isCurrent) setLoadError(projectErrorMessage(error, t));
      });
    return () => {
      isCurrent = false;
    };
  }, [t]);

  /** Closes on Escape. */
  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
    }
  }

  return (
    <div
      className="fixed inset-0 z-50 flex items-end justify-center bg-slate-900/50 p-4 sm:items-center"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose();
      }}
    >
      <div
        role="dialog"
        aria-modal="true"
        aria-labelledby={headingId}
        onKeyDown={handleKeyDown}
        className="w-full max-w-md rounded-2xl bg-surface p-6 shadow-xl"
      >
        <h2 id={headingId} className="text-lg font-semibold">{t("addToAProject", { title: meeting.title })}</h2>
        {projects !== null ? (
          <MeetingProjectPicker meetingId={meeting.meeting_id} projectId={meeting.project_id ?? null} projects={projects} />
        ) : (
          <p className="my-3 text-muted">{loadError ?? t("loadingProjects")}</p>
        )}
        <div className="mt-4 flex justify-end">
          <button
            type="button"
            autoFocus
            onClick={onClose}
            className={buttonStyles.outlined}
          >{t("done")}</button>
        </div>
      </div>
    </div>
  );
}
