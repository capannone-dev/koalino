// Renders the modal the meeting list's selection bar opens from "Move to project" (#216): it loads
// the projects, lets the person choose one (or none), and moves every selected meeting there.

"use client";

import { KeyboardEvent, useEffect, useId, useState } from "react";
import { getProjects, moveMeetingToProject, projectErrorMessage, projectTree, type ProjectSummary } from "@/lib/projects-api";
import { useT } from "@/lib/i18n/client";

/** The choice that removes the meetings from their projects; no project ID is empty. */
const noProjectValue = "";

/**
 * Moves several meetings into one project.
 *
 * Inputs: the meeting IDs, `onClose` (cancelled), and `onMoved` with the message to announce.
 * Outputs: a modal dialog with a project list and Move. Side effects: loads the projects, then
 * sends one move per meeting; a failed move is reported and the others still happen.
 */
export function BatchProjectDialog({
  meetingIds,
  onClose,
  onMoved,
}: {
  meetingIds: string[];
  onClose: () => void;
  onMoved: (message: string) => void;
}) {
  const t = useT();
  const [projects, setProjects] = useState<ProjectSummary[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [target, setTarget] = useState(noProjectValue);
  const [isMoving, setIsMoving] = useState(false);
  const headingId = useId();
  const selectId = useId();

  useEffect(() => {
    let isCurrent = true;
    getProjects()
      .then((loaded) => {
        if (!isCurrent) return;
        setProjects(loaded);
        if (loaded.length > 0) setTarget(projectTree(loaded)[0].project.project_id);
      })
      .catch((failure: unknown) => {
        if (isCurrent) setError(projectErrorMessage(failure, t));
      });
    return () => {
      isCurrent = false;
    };
  }, [t]);

  /** Sends the moves and reports how they went. */
  async function move() {
    setIsMoving(true);
    let failed = 0;
    for (const meetingId of meetingIds) {
      try {
        await moveMeetingToProject(meetingId, target === noProjectValue ? null : target);
      } catch {
        failed++;
      }
    }
    setIsMoving(false);
    if (failed > 0) {
      setError(t("someMeetingsCouldNotBeMoved"));
      return;
    }
    const name = projects?.find((project) => project.project_id === target)?.name ?? "";
    onMoved(
      target === noProjectValue
        ? t("removedMeetingsFromTheirProjects", { count: meetingIds.length })
        : t("movedMeetingsTo", { count: meetingIds.length, project: name })
    );
  }

  /** Closes on Escape. */
  function handleKeyDown(event: KeyboardEvent<HTMLDivElement>) {
    if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
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
        <h2 id={headingId} className="text-lg font-semibold">
          {t("moveToProject")}
        </h2>
        <p className="mt-1 text-muted">{t("selectedCount", { count: meetingIds.length })}</p>
        {projects === null && error === null ? <p className="my-3 text-muted">{t("loadingProjects")}</p> : null}
        {projects !== null ? (
          <div className="my-3">
            <label htmlFor={selectId} className="sr-only">
              {t("moveToProject")}
            </label>
            <select
              id={selectId}
              autoFocus
              value={target}
              onChange={(event) => setTarget(event.target.value)}
              className="min-h-11 w-full rounded-lg border border-line-strong bg-surface px-3"
            >
              {projectTree(projects).map(({ project, depth }) => (
                <option key={project.project_id} value={project.project_id}>
                  {`${"  ".repeat(depth)}${project.name}`}
                </option>
              ))}
              <option value={noProjectValue}>{t("noProject")}</option>
            </select>
          </div>
        ) : null}
        {error ? (
          <p role="alert" className="my-3 text-danger">
            {error}
          </p>
        ) : null}
        <div className="mt-4 flex justify-end gap-2">
          <button type="button" onClick={onClose} className="min-h-11 rounded-lg border border-line-strong px-4 font-medium hover:bg-surface-muted">
            {t("cancel")}
          </button>
          <button
            type="button"
            disabled={projects === null || isMoving}
            onClick={move}
            className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:opacity-50"
          >
            {t("move")}
          </button>
        </div>
      </div>
    </div>
  );
}
