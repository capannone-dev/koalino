// Moves a meeting into a project, or out of any (Issue #85), from the meeting's page.

"use client";

import { useId, useState } from "react";
import { moveMeetingToProject, projectErrorMessage, type ProjectSummary } from "@/lib/projects-api";

/**
 * A "Project" selector with "No project" first.
 *
 * Inputs: the meeting, its current project (`null` outside any) and the projects. Outputs: the
 * selector with a status line. Side effects: `PUT /api/meetings/{id}/project` on each choice; a
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
  const [currentProjectId, setCurrentProjectId] = useState(projectId);
  const [message, setMessage] = useState("");
  const [isFailure, setIsFailure] = useState(false);
  const [isMoving, setIsMoving] = useState(false);
  const selectId = useId();

  /** Sends the move, keeping the old choice when it fails. */
  async function choose(value: string) {
    const target = value === "" ? null : value;
    const previous = currentProjectId;
    setCurrentProjectId(target);
    setIsMoving(true);
    setMessage("");
    try {
      await moveMeetingToProject(meetingId, target);
      setIsFailure(false);
      setMessage(target === null ? "Removed from the project" : "Moved to the project");
    } catch (error) {
      setCurrentProjectId(previous);
      setIsFailure(true);
      setMessage(projectErrorMessage(error));
    } finally {
      setIsMoving(false);
    }
  }

  return (
    <div className="my-3 max-w-md">
      <label htmlFor={selectId} className="block text-sm font-medium">
        Project
      </label>
      <select
        id={selectId}
        value={currentProjectId ?? ""}
        disabled={isMoving}
        onChange={(event) => choose(event.target.value)}
        className="mt-1 block min-h-11 w-full rounded-lg border border-line-strong px-3 text-base focus:border-focus focus:outline-none focus:ring-2 focus:ring-focus/30"
      >
        <option value="">No project</option>
        {projects.map((project) => (
          <option key={project.project_id} value={project.project_id}>
            {project.name}
          </option>
        ))}
      </select>
      <p role="status" aria-live="polite" className={`text-sm ${isFailure ? "text-danger" : "text-success"}`}>
        {message}
      </p>
    </div>
  );
}
