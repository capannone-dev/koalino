// Moves a meeting into a project, or out of any (Issue #85), from the meeting's page and from the
// meeting list's row menu (#94). "New project…" names a project, creates it and moves the meeting
// into it in one step.

"use client";

import { FormEvent, useId, useState } from "react";
import {
  createProject,
  moveMeetingToProject,
  projectErrorMessage,
  projectNameError,
  type ProjectSummary
} from "@/lib/projects-api";
import { useT } from "@/lib/i18n/client";

/** The selector's value for "New project…". */
const newProjectValue = "__new__";

/**
 * A "Project" selector with "No project" first and "New project…" last.
 *
 * Inputs: the meeting, its current project (`null` outside any) and the projects. Outputs: the
 * selector, a name field while a new project is being named, and a status line. Side effects:
 * `PUT /api/meetings/{id}/project` on each choice, after `POST /api/projects` for a new one; a
 * failure puts the previous choice back.
 */
export function MeetingProjectPicker({
  meetingId,
  projectId,
  projects
}: {
  meetingId: string;
  projectId: string | null;
  projects: ProjectSummary[];
}) {
  const t = useT();
  const [currentProjectId, setCurrentProjectId] = useState(projectId);
  const [knownProjects, setKnownProjects] = useState(projects);
  const [isNaming, setIsNaming] = useState(false);
  const [newName, setNewName] = useState("");
  const [nameError, setNameError] = useState<string | null>(null);
  const [message, setMessage] = useState("");
  const [isFailure, setIsFailure] = useState(false);
  const [isMoving, setIsMoving] = useState(false);
  const selectId = useId();
  const nameId = useId();

  /** Sends the move, keeping the old choice when it fails. */
  async function moveTo(target: string | null, targetName: string | null) {
    const previous = currentProjectId;
    setCurrentProjectId(target);
    setIsMoving(true);
    setMessage("");
    try {
      await moveMeetingToProject(meetingId, target);
      setIsFailure(false);
      setMessage(target === null ? t("removedFromTheProject") : t("movedTo", { project: targetName ?? "the project" }));
    } catch (error) {
      setCurrentProjectId(previous);
      setIsFailure(true);
      setMessage(projectErrorMessage(error, t));
    } finally {
      setIsMoving(false);
    }
  }

  /** Handles a choice: a project, no project, or naming a new one. */
  function choose(value: string) {
    if (value === newProjectValue) {
      setIsNaming(true);
      setMessage("");
      return;
    }
    setIsNaming(false);
    const target = value === "" ? null : value;
    void moveTo(target, knownProjects.find((project) => project.project_id === target)?.name ?? null);
  }

  /** Creates the named project and moves the meeting into it. */
  async function createAndMove(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const error = projectNameError(newName, t);
    setNameError(error);
    if (error !== null) return;
    setIsMoving(true);
    try {
      const created = await createProject(newName.trim());
      setKnownProjects((current) => [...current, created]);
      setIsNaming(false);
      setNewName("");
      await moveTo(created.project_id, created.name);
    } catch (failure) {
      setIsFailure(true);
      setMessage(projectErrorMessage(failure, t));
      setIsMoving(false);
    }
  }

  return (
    <div className="my-3 max-w-md">
      <label htmlFor={selectId} className="block text-sm font-medium">{t("project")}</label>
      <select
        id={selectId}
        value={isNaming ? newProjectValue : (currentProjectId ?? "")}
        disabled={isMoving}
        onChange={(event) => choose(event.target.value)}
        className="mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30"
      >
        <option value="">{t("noProject")}</option>
        {knownProjects.map((project) => (
          <option key={project.project_id} value={project.project_id}>
            {project.name}
          </option>
        ))}
        <option value={newProjectValue}>{t("newProject")}</option>
      </select>
      {isNaming ? (
        <form onSubmit={createAndMove} noValidate className="mt-2 flex flex-wrap items-end gap-2">
          <div className="min-w-0 flex-1">
            <label htmlFor={nameId} className="block text-sm font-medium">{t("newProjectName")}</label>
            <input
              id={nameId}
              type="text"
              autoFocus
              value={newName}
              disabled={isMoving}
              aria-invalid={nameError !== null}
              onChange={(event) => {
                setNewName(event.target.value);
                setNameError(null);
              }}
              className="mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30 aria-invalid:border-danger"
            />
          </div>
          <button
            type="submit"
            disabled={isMoving}
            className="min-h-11 rounded-lg bg-accent px-4 font-medium text-on-accent hover:bg-accent-strong disabled:bg-disabled"
          >{t("createAndMove")}</button>
          <p role="alert" className="w-full text-sm text-danger">
            {nameError}
          </p>
        </form>
      ) : null}
      <p role="status" aria-live="polite" className={`text-sm ${isFailure ? "text-danger" : "text-success"}`}>
        {message}
      </p>
    </div>
  );
}
